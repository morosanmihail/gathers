<script lang="ts">
	// A deck's page. Separate from the collection page rather than a mode of it: a deck has boards
	// but no prices, wants, purchase history, or pagination, so sharing the route would have meant a
	// screen full of controls that don't apply.
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import DeckBoard from '$lib/components/DeckBoard.svelte';
	import SearchModal from '$lib/components/SearchModal.svelte';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import {
		getDeckCards, adjustDeckCard, moveDeckCard,
		renameCollection, deleteCollection, invalidateCache
	} from '$lib/api';
	import { app } from '$lib/state.svelte';
	import { boardsInUse, boardsToOffer, boardLabel } from '$lib/types';
	import type { DeckCard } from '$lib/types';

	const deckId = $derived(decodeURIComponent($page.params.id ?? ''));

	let cards = $state<DeckCard[]>([]);
	let loading = $state(true);
	let searchOpen = $state(false);
	let addBoard = $state('main');
	let renaming = $state(false);
	let renameValue = $state('');
	let renameError = $state('');
	// Quantity and board edits are the only mutations here with nowhere to report a failure: the
	// row simply wouldn't move, which reads as an unresponsive button rather than a refusal.
	let actionError = $state('');
	let loadError = $state('');
	let confirmDelete = $state(false);

	const boards = $derived(boardsInUse(cards));
	// Wider than what's rendered, so a sideboard can be started
	const addBoards = $derived(boardsToOffer(cards));
	const total = $derived(cards.reduce((n, c) => n + (c.quantity ?? 0), 0));

	function cardsOn(board: string): DeckCard[] {
		return cards.filter((c) => (c.board ?? '') === board);
	}

	async function load() {
		loading = true;
		loadError = '';
		try {
			cards = await getDeckCards(deckId);
		} catch (e) {
			// Unlike a collection's card list, these endpoints genuinely refuse: 404 for a name
			// that isn't a deck any more, 400 for a collection's name. Without this the page
			// renders "no cards in this deck yet", which is a different claim entirely.
			console.error('[gathers] deck load failed:', e);
			cards = [];
			loadError = String(e);
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		if (app.ready && !app.collectionsEnabled) {
			goto('/search', { replaceState: true });
			return;
		}
		deckId;
		load();
	});

	async function adjust(card: DeckCard, delta: number) {
		actionError = '';
		try {
			await adjustDeckCard(deckId, card.id, card.finish ?? '', card.board ?? '', delta, card.provider);
			cards = cards
				.map((c) =>
					c.id === card.id && (c.finish ?? '') === (card.finish ?? '') && (c.board ?? '') === (card.board ?? '')
						? { ...c, quantity: Math.max(0, (c.quantity ?? 0) + delta) }
						: c
				)
				.filter((c) => (c.quantity ?? 0) > 0);
		} catch (e) {
			console.error('[gathers] deck adjust failed:', e);
			actionError = `Couldn't change that quantity. ${e}`;
		}
	}

	async function move(card: DeckCard, toBoard: string) {
		actionError = '';
		try {
			await moveDeckCard(deckId, card.id, card.finish ?? '', card.board ?? '', toBoard, 1);
			// The server moved one copy between two rows; reload rather than reproduce that
			// arithmetic, since the destination row may be new.
			cards = await getDeckCards(deckId);
		} catch (e) {
			console.error('[gathers] deck move failed:', e);
			actionError = `Couldn't move that card. ${e}`;
		}
	}

	// Renaming and deleting are collection operations: a deck is a collection with a kind, and
	// shares that plumbing. They live here rather than in CollectionToolbar because that toolbar
	// also carries share links, purchase history, and filters, none of which mean anything for a
	// deck.
	function startRename() {
		renameValue = deckId;
		renameError = '';
		renaming = true;
	}

	async function handleRename(e: Event) {
		e.preventDefault();
		const newName = renameValue.trim();
		if (!newName || newName === deckId) { renaming = false; return; }
		renameError = '';
		try {
			await app.withOp('Renaming deck', () => renameCollection(deckId, newName));
			invalidateCache('collections');
			await app.loadCollections();
			renaming = false;
			// The deck's cards follow the rename (the foreign keys on deck_cards cascade), so the
			// new page is the same deck under a new name.
			goto(`/deck/${encodeURIComponent(newName)}`);
		} catch (err) {
			renameError = String(err);
		}
	}

	async function handleDelete() {
		confirmDelete = false;
		try {
			await app.withOp('Deleting deck', () => deleteCollection(deckId));
			invalidateCache('collections');
			await app.loadCollections();
			goto('/');
		} catch (e) {
			console.error('[gathers] deck delete failed:', e);
		}
	}
</script>

<svelte:head>
	<title>{deckId} - gatheRs</title>
</svelte:head>

<div>
	<div class="toolbar">
		<button class="btn" onclick={() => (searchOpen = !searchOpen)}>
			{searchOpen ? 'Close search' : 'Search & add'}
		</button>
		<label class="deck-add-board">
			Add to
			<select class="select" bind:value={addBoard}>
				{#each addBoards as b (b)}
					<option value={b}>{boardLabel(b)}</option>
				{/each}
			</select>
		</label>

		<div class="toolbar-sep"></div>

		{#if renaming}
			<form style="display:flex; gap:6px; align-items:center;" onsubmit={handleRename}>
				<!-- svelte-ignore a11y_autofocus -->
				<input
					class="input"
					style="height:34px; padding:4px 10px; width:160px;"
					bind:value={renameValue}
					autofocus
				/>
				<button type="submit" class="btn btn-accent">Save</button>
				<button type="button" class="btn btn-ghost" onclick={() => renaming = false}>✕</button>
				{#if renameError}<span style="color:var(--danger);font-size:0.78rem;">{renameError}</span>{/if}
			</form>
		{:else}
			<button class="btn" onclick={startRename} title="Rename deck">
				<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
					<path d="M2 10.5l1-3L9.5 1.5a1.41 1.41 0 012 2L5 10l-3 .5z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
					<path d="M8 3l2 2" stroke="currentColor" stroke-width="1.3"/>
				</svg>
				Rename
			</button>
		{/if}

		<button class="btn btn-danger" onclick={() => confirmDelete = true}>
			<svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
				<path d="M2 4h10M5 4V3a1 1 0 011-1h2a1 1 0 011 1v1M11 4l-.9 8.1A1 1 0 019.1 13H4.9a1 1 0 01-1-.9L3 4" stroke="currentColor" stroke-width="1.2" fill="none"/>
			</svg>
			Delete deck
		</button>
	</div>

	<div class="page-header" style="padding-bottom: 8px;">
		<h1 class="page-title">{deckId}</h1>
		{#if !loading}
			<span class="page-subtitle">{total.toLocaleString()} card{total !== 1 ? 's' : ''}</span>
		{/if}
	</div>

	{#if actionError}
		<div class="deck-action-error" role="alert">
			{actionError}
			<button type="button" class="btn btn-sm btn-ghost" onclick={() => actionError = ''}>✕</button>
		</div>
	{/if}

	{#if loading && cards.length === 0}
		<div class="loading-row"><div class="spinner"></div> Loading…</div>
	{:else if loadError}
		<div class="empty-state">
			<div class="empty-state-icon">⚠</div>
			<div class="empty-state-text">Couldn't load this deck. {loadError}</div>
		</div>
	{:else if !loading && cards.length === 0}
		<div class="empty-state">
			<div class="empty-state-icon">🂠</div>
			<div class="empty-state-text">No cards in this deck yet. Use Search &amp; add above.</div>
		</div>
	{:else}
		<div class="deck-boards">
			{#each boards as board (board)}
				<DeckBoard
					{board}
					cards={cardsOn(board)}
					boards={addBoards}
					onAdjust={adjust}
					onMove={move}
				/>
			{/each}
		</div>
	{/if}
</div>

{#if searchOpen}
	<SearchModal
		collection={deckId}
		deckBoard={addBoard}
		onclose={() => (searchOpen = false)}
		onAdded={load}
	/>
{/if}

{#if confirmDelete}
	<ConfirmDialog
		title="Delete deck"
		message="Delete '{deckId}' and its card list? The cards stay in whatever collections you own them in; a deck only records what you intended to play."
		confirmLabel="Delete"
		danger
		onconfirm={handleDelete}
		oncancel={() => (confirmDelete = false)}
	/>
{/if}
