<script lang="ts">
	// A deck's page. Separate from the collection page rather than a mode of it: a deck has boards
	// and a build state but no prices, purchase history, or pagination, so sharing the route would
	// have meant a screen full of controls that don't apply.
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import DeckBoard from '$lib/components/DeckBoard.svelte';
	import SearchModal from '$lib/components/SearchModal.svelte';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import ShareLinksModal from '$lib/components/ShareLinksModal.svelte';
	import CardDetailModal from '$lib/components/CardDetailModal.svelte';
	import ConvertedTotal from '$lib/components/ConvertedTotal.svelte';
	import { formatMoney, formatMoneyList } from '$lib/currency.svelte';
	import {
		getDeckCards, adjustDeckCard, moveDeckCard, transferDeckCard,
		getDeckBoards, addDeckBoard, renameDeckBoard, deleteDeckBoard, reorderDeckBoards,
		getDeckMeta, setDeckMeta, getCardAvailability, getDeckValue,
		renameCollection, deleteCollection, setCollectionKind, invalidateCache
	} from '$lib/api';
	import { app } from '$lib/state.svelte';
	import { deckSlots, isDeck } from '$lib/types';
	import type {
		CardAvailability,
		DeckBoard as DeckBoardInfo,
		DeckCard,
		DeckMeta,
		TransferDirection,
		ValueBreakdown
	} from '$lib/types';

	const deckId = $derived(decodeURIComponent($page.params.id ?? ''));

	let cards = $state<DeckCard[]>([]);
	let boardInfo = $state<DeckBoardInfo[]>([]);
	let meta = $state<DeckMeta>({ built: false, location: null, notes: null });
	let availability = $state<Record<string, CardAvailability>>({});
	let loading = $state(true);
	let searchOpen = $state(false);
	let detailCard = $state<DeckCard | null>(null);
	let valueBreakdown = $state<ValueBreakdown | null>(null);
	let valueHover = $state(false);
	const currencyTotals = $derived(
		(valueBreakdown?.currencies ?? []).map((c) => ({ currency: c.currency, value: c.total_value }))
	);
	// Only the copies still missing: a want is what the owned copies count towards, so wanting
	// four and having three is one card short, not four
	const toCompleteTotals = $derived(
		(valueBreakdown?.currencies ?? []).map((c) => ({ currency: c.currency, value: c.wanted_value }))
	);
	let addBoard = $state('');
	let locationDraft = $state('');
	let savingMeta = $state(false);
	let renaming = $state(false);
	let renameValue = $state('');
	let renameError = $state('');
	// Quantity and board edits are the only mutations here with nowhere to report a failure: the
	// row simply wouldn't move, which reads as an unresponsive button rather than a refusal.
	let actionError = $state('');
	let loadError = $state('');
	let confirmDelete = $state(false);
	let managingBoards = $state(false);
	let newBoardName = $state('');
	let editingBoard = $state<string | null>(null);
	let boardNameDraft = $state('');
	let confirmBoardDelete = $state<string | null>(null);
	let confirmConvert = $state(false);
	let shareOpen = $state(false);

	// The server owns both the names and the order, so these are its answer rather than something
	// derived from the rows: an empty board is still a board, and the user can reorder them.
	const boards = $derived(boardInfo.map((b) => b.board));
	// Wanting four and holding three is four cards of the deck, not seven.
	const total = $derived(cards.reduce((n, c) => n + deckSlots(c), 0));
	const collections = $derived(app.collections.filter((c) => !isDeck(c)).map((c) => c.id));

	function cardsOn(board: string): DeckCard[] {
		return cards.filter((c) => (c.board ?? '') === board);
	}

	async function load() {
		loading = true;
		loadError = '';
		try {
			const [deckCards, deckBoards, deckMeta] = await Promise.all([
				getDeckCards(deckId),
				getDeckBoards(deckId),
				getDeckMeta(deckId)
			]);
			cards = deckCards;
			boardInfo = deckBoards;
			if (!addBoard || !deckBoards.some((b) => b.board === addBoard)) {
				addBoard = deckBoards[0]?.board ?? '';
			}
			meta = deckMeta;
			locationDraft = deckMeta.location ?? '';
			// Non-blocking, like a collection's: prices can take a while and the list doesn't need them
			refreshValue();
			// One lookup for the whole list rather than one per row.
			availability = await getCardAvailability([...new Set(deckCards.map((c) => c.id))]);
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

	function refreshValue() {
		if (app.pricingEnabled) {
			getDeckValue(deckId).then((v) => { valueBreakdown = v; });
		}
	}

	async function refreshAvailability() {
		availability = await getCardAvailability([...new Set(cards.map((c) => c.id))]);
	}

	function sameRow(a: DeckCard, b: DeckCard): boolean {
		return a.id === b.id && (a.finish ?? '') === (b.finish ?? '') && (a.board ?? '') === (b.board ?? '');
	}

	/** Changes one of the row's two counts; `field` picks which. */
	async function adjust(card: DeckCard, delta: number, field: 'quantity' | 'wantQuantity' = 'quantity') {
		actionError = '';
		const want = field === 'wantQuantity';
		try {
			await adjustDeckCard(
				deckId,
				card.id,
				card.finish ?? '',
				card.board ?? '',
				want ? 0 : delta,
				card.provider,
				want ? delta : 0
			);
			cards = cards
				.map((c) =>
					sameRow(c, card) ? { ...c, [field]: Math.max(0, (c[field] ?? 0) + delta) } : c
				)
				// A row survives until both counts are gone: giving up the last owned copy leaves
				// the want standing.
				.filter((c) => (c.quantity ?? 0) > 0 || (c.wantQuantity ?? 0) > 0);
			refreshValue();
			// A built deck's contents change what every card is short by.
			if (meta.built) refreshAvailability();
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

	/**
	 * Moves copies between a collection and this deck.
	 *
	 * Unlike `adjust`, the copies leave the collection, so the deck's own count isn't the only
	 * thing that changed and the list is reloaded wholesale.
	 */
	async function transfer(card: DeckCard, collection: string, direction: TransferDirection) {
		actionError = '';
		try {
			const moved = await transferDeckCard(
				deckId, collection, card.id, card.finish ?? '', card.board ?? '', direction
			);
			if (moved === 0) {
				actionError = direction === 'intoDeck'
					? `'${collection}' hasn't got a copy of ${card.name} to move.`
					: `This board hasn't got a copy of ${card.name} to give back.`;
				return;
			}
			cards = await getDeckCards(deckId);
			refreshValue();
			await refreshAvailability();
		} catch (e) {
			console.error('[gathers] deck transfer failed:', e);
			actionError = `Couldn't move that card. ${e}`;
		}
	}

	// ── boards ───────────────────────────────────────────────────────────────
	//
	// Every board mutation returns the deck's whole board list, so the local copy is replaced
	// rather than patched. Renames and deletions also move cards, so those reload the rows too.

	async function handleAddBoard(e: Event) {
		e.preventDefault();
		const name = newBoardName.trim();
		if (!name) return;
		actionError = '';
		try {
			boardInfo = await addDeckBoard(deckId, name);
			newBoardName = '';
		} catch (err) {
			actionError = `Couldn't add that board. ${err}`;
		}
	}

	async function handleRenameBoard(from: string) {
		const to = boardNameDraft.trim();
		editingBoard = null;
		if (!to || to === from) return;
		actionError = '';
		try {
			boardInfo = await renameDeckBoard(deckId, from, to);
			cards = await getDeckCards(deckId);
			if (addBoard === from) addBoard = to;
		} catch (err) {
			actionError = `Couldn't rename that board. ${err}`;
		}
	}

	async function handleDeleteBoard(board: string) {
		confirmBoardDelete = null;
		actionError = '';
		try {
			boardInfo = await deleteDeckBoard(deckId, board);
			cards = await getDeckCards(deckId);
			if (addBoard === board) addBoard = boardInfo[0]?.board ?? '';
			refreshValue();
			if (meta.built) await refreshAvailability();
		} catch (err) {
			actionError = `Couldn't delete that board. ${err}`;
		}
	}

	async function moveBoard(board: string, delta: number) {
		const order = boards.slice();
		const from = order.indexOf(board);
		const to = from + delta;
		if (from < 0 || to < 0 || to >= order.length) return;
		order.splice(to, 0, ...order.splice(from, 1));
		actionError = '';
		try {
			boardInfo = await reorderDeckBoards(deckId, order);
		} catch (err) {
			actionError = `Couldn't reorder the boards. ${err}`;
		}
	}

	// Turns the deck back into a plain collection, collapsing its boards into one pile
	async function handleConvert() {
		confirmConvert = false;
		try {
			await app.withOp('Converting to collection', () => setCollectionKind(deckId, 'collection'));
			invalidateCache('collections');
			await app.loadCollections();
			goto(`/collection/${encodeURIComponent(deckId)}`);
		} catch (e) {
			console.error('[gathers] deck convert failed:', e);
			actionError = `Couldn't convert this deck. ${e}`;
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
			// The deck's cards and build state follow the rename (the foreign keys on deck_cards
			// and deck_meta cascade), so the new page is the same deck under a new name.
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

	async function saveMeta(next: Partial<DeckMeta>) {
		actionError = '';
		savingMeta = true;
		try {
			meta = await setDeckMeta(deckId, {
				built: next.built ?? meta.built,
				location: next.location !== undefined ? next.location : meta.location,
				notes: meta.notes
			});
			locationDraft = meta.location ?? '';
			await refreshAvailability();
		} catch (e) {
			console.error('[gathers] deck meta save failed:', e);
			actionError = `Couldn't save the deck's build state. ${e}`;
		} finally {
			savingMeta = false;
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
				{#each boards as b (b)}
					<option value={b}>{b}</option>
				{/each}
			</select>
		</label>

		<button class="btn" onclick={() => (managingBoards = !managingBoards)}>
			{managingBoards ? 'Done with boards' : 'Edit boards'}
		</button>

		<label class="deck-built-toggle" title="A built deck is assembled in real life, so its cards count as committed">
			<input
				type="checkbox"
				checked={meta.built}
				disabled={savingMeta}
				onchange={(e) => saveMeta({ built: e.currentTarget.checked })}
			/>
			Built
		</label>

		{#if meta.built}
			<input
				class="input deck-location-input"
				placeholder="Where is it? e.g. red deck box"
				bind:value={locationDraft}
				disabled={savingMeta}
				onblur={() => locationDraft !== (meta.location ?? '') && saveMeta({ location: locationDraft || null })}
				onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
			/>
		{/if}

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

		<!-- A deck is a collection with a kind, so share links work on it unchanged -->
		<button class="btn" onclick={() => shareOpen = true} title="Manage shareable links">
			<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
				<circle cx="3" cy="7" r="1.8" stroke="currentColor" stroke-width="1.3"/>
				<circle cx="11" cy="3" r="1.8" stroke="currentColor" stroke-width="1.3"/>
				<circle cx="11" cy="11" r="1.8" stroke="currentColor" stroke-width="1.3"/>
				<path d="M4.6 6.2L9.4 3.8M4.6 7.8L9.4 10.2" stroke="currentColor" stroke-width="1.3"/>
			</svg>
			Share
		</button>

		<button class="btn" onclick={() => confirmConvert = true} title="Make this a plain collection">
			Convert to collection
		</button>

		<button class="btn btn-danger" onclick={() => confirmDelete = true}>
			<svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
				<path d="M2 4h10M5 4V3a1 1 0 011-1h2a1 1 0 011 1v1M11 4l-.9 8.1A1 1 0 019.1 13H4.9a1 1 0 01-1-.9L3 4" stroke="currentColor" stroke-width="1.2" fill="none"/>
			</svg>
			Delete deck
		</button>
	</div>

	{#if managingBoards}
		<!-- Board names are free-form, so this is a plain text field rather than a fixed set -->
		<div class="deck-board-manager">
			<form class="deck-board-add" onsubmit={handleAddBoard}>
				<input
					class="input"
					placeholder="New board, e.g. Maybeboard or Mainboard:Lands"
					bind:value={newBoardName}
				/>
				<button type="submit" class="btn btn-accent" disabled={!newBoardName.trim()}>Add board</button>
			</form>

			<ul class="deck-board-list">
				{#each boards as b, i (b)}
					<li class="deck-board-row">
						{#if editingBoard === b}
							<!-- svelte-ignore a11y_autofocus -->
							<input
								class="input"
								bind:value={boardNameDraft}
								autofocus
								onblur={() => handleRenameBoard(b)}
								onkeydown={(e) => {
									if (e.key === 'Enter') e.currentTarget.blur();
									if (e.key === 'Escape') editingBoard = null;
								}}
							/>
						{:else}
							<span class="deck-board-row-name">{b}</span>
							<span class="deck-board-row-count">
								{cardsOn(b).reduce((n, c) => n + deckSlots(c), 0)}
							</span>
						{/if}

						<button
							class="btn btn-sm btn-ghost"
							disabled={i === 0}
							title="Move up"
							onclick={() => moveBoard(b, -1)}
						>↑</button>
						<button
							class="btn btn-sm btn-ghost"
							disabled={i === boards.length - 1}
							title="Move down"
							onclick={() => moveBoard(b, 1)}
						>↓</button>
						<button
							class="btn btn-sm"
							onclick={() => { editingBoard = b; boardNameDraft = b; }}
						>Rename</button>
						<button
							class="btn btn-sm btn-danger"
							onclick={() => confirmBoardDelete = b}
						>Delete</button>
					</li>
				{/each}
			</ul>
		</div>
	{/if}

	<div class="page-header" style="padding-bottom: 8px;">
		<h1 class="page-title">{deckId}</h1>
		{#if !loading}
			<span class="page-subtitle">{total.toLocaleString()} card{total !== 1 ? 's' : ''}</span>
			{#if meta.built}
				<span class="deck-badge deck-badge-built">
					Built{meta.location ? ` (${meta.location})` : ''}
				</span>
			{/if}
		{/if}
		{#if valueBreakdown}
			<span
				class="page-subtitle"
				role="status"
				aria-label="Deck value"
				style="margin-left: auto; position: relative;"
				onmouseenter={() => valueHover = true}
				onmouseleave={() => valueHover = false}
			>
				≈ {formatMoneyList(currencyTotals) || formatMoney(0)}
				<ConvertedTotal items={currencyTotals} />
				{#if valueHover}
					<!-- No profit rows, unlike a collection's: purchase history stays with the
					     collection a copy was bought into -->
					<div class="value-breakdown-tooltip">
						{#if valueBreakdown.priced_count != null && valueBreakdown.total_count != null}
							<div class="vb-row">
								<span class="vb-label">Priced</span>
								<span class="vb-val">{valueBreakdown.priced_count} / {valueBreakdown.total_count}</span>
							</div>
						{/if}
						{#if toCompleteTotals.some((m) => m.value > 0)}
							<div class="vb-row">
								<span class="vb-label" title="Price of the wanted copies the deck doesn't have yet">To complete</span>
								<span class="vb-val">{formatMoneyList(toCompleteTotals)}</span>
							</div>
						{/if}
					</div>
				{/if}
			</span>
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
					deck={deckId}
					{board}
					cards={cardsOn(board)}
					{availability}
					{boards}
					{collections}
					onAdjust={adjust}
					onMove={move}
					onTransfer={transfer}
					onclick={(c) => (detailCard = c)}
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

{#if confirmBoardDelete}
	<ConfirmDialog
		title="Delete board"
		message="Delete '{confirmBoardDelete}' and the {cardsOn(confirmBoardDelete).length} card{cardsOn(confirmBoardDelete).length === 1 ? '' : 's'} on it? The cards stay in whatever collections you own them in."
		confirmLabel="Delete"
		danger
		onconfirm={() => handleDeleteBoard(confirmBoardDelete!)}
		oncancel={() => (confirmBoardDelete = null)}
	/>
{/if}

{#if confirmConvert}
	<ConfirmDialog
		title="Convert to collection"
		message="Turn '{deckId}' into a plain collection? Its boards collapse into one pile and its build state is dropped, since a collection is never assembled. The cards come with it."
		confirmLabel="Convert"
		onconfirm={handleConvert}
		oncancel={() => (confirmConvert = false)}
	/>
{/if}

{#if shareOpen}
	<ShareLinksModal collection={deckId} onclose={() => shareOpen = false} />
{/if}

{#if detailCard}
	<CardDetailModal card={detailCard} onclose={() => (detailCard = null)} />
{/if}
