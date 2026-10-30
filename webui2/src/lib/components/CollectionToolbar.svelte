<script lang="ts">
	import ConfirmDialog from './ConfirmDialog.svelte';
	import CsvExportModal from './CsvExportModal.svelte';
	import CsvImportModal from './CsvImportModal.svelte';
	import { app } from '$lib/state.svelte';
	import { goto } from '$app/navigation';
	import {
		deleteCollection,
		deleteCardFromCollection,
		moveCards,
		renameCollection,
		setCollectionKind,
		invalidateCache
	} from '$lib/api';
	import { isDeck } from '$lib/types';
	import type { CardGroup } from '$lib/types';

	interface Props {
		collection: string;
		groups: CardGroup[];
		onRefresh: () => void;
		onSearchOpen: () => void;
		searchOpen: boolean;
		onHistoryOpen?: () => void;
		onShareOpen?: () => void;
		onFilterOpen?: () => void;
		filterOpen?: boolean;
		filterActive?: boolean;
	}

	let { collection, groups, onRefresh, onSearchOpen, searchOpen, onHistoryOpen, onShareOpen, onFilterOpen, filterOpen = false, filterActive = false }: Props = $props();

	let confirmDelete = $state<'collection' | 'cards' | null>(null);
	let confirmConvert = $state(false);
	let moveDest = $state('');
	let moveError = $state('');
	// The file picked for import, waiting on its column mapping.
	let importFile = $state<File | null>(null);
	let exportOpen = $state(false);
	let renaming = $state(false);
	let renameValue = $state('');
	let renameError = $state('');

	const selectedList = $derived(
		groups.filter(g => app.selectedCards.has(g.id))
	);

	const removable = $derived(
		app.collections.find(c => c.id === collection)?.removable ?? true
	);

	// Decks are excluded: cards move between things that record ownership, and moving into a deck
	// would write rows the deck view never reads, so they'd leave this collection and appear
	// nowhere. The server refuses it too.
	const otherCollections = $derived(
		app.collections.filter(c => c.id !== collection && !isDeck(c))
	);

	$effect(() => {
		if (otherCollections.length > 0 && !moveDest) {
			moveDest = otherCollections[0].id;
		}
	});

	async function handleDeleteCollection() {
		confirmDelete = null;
		try {
			await app.withOp('Deleting collection', () => deleteCollection(collection));
			invalidateCache('collections');
			await app.loadCollections();
			goto('/');
		} catch (e) {
			console.error(e);
		}
	}

	async function handleDeleteCards() {
		confirmDelete = null;
		try {
			await app.withOp('Deleting cards', async () => {
				for (const group of selectedList) {
					for (const entry of group.entries) {
						if ((entry.quantity ?? 0) > 0) {
							await deleteCardFromCollection(collection, entry.id, entry.finish ?? '', entry.quantity ?? 0);
						}
					}
				}
			});
			app.clearSelected();
			onRefresh();
		} catch (e) {
			console.error(e);
		}
	}

	async function handleMove() {
		if (!moveDest || !selectedList.length) return;
		moveError = '';
		try {
			// Every finish row of each selected card moves — including a
			// want-only "" row that has no owned copies of anything.
			const items = selectedList.flatMap(g => g.entries.map(entry => ({
				id: entry.id,
				finish: entry.finish ?? '',
				quantity: entry.quantity ?? 0,
				wantQuantity: entry.wantQuantity ?? 0,
				collectionId: entry.collectionId,
				provider: entry.provider ?? ''
			})));
			await app.withOp(`Moving to ${moveDest}`, () => moveCards(moveDest, items));
			app.clearSelected();
			onRefresh();
		} catch (e) {
			moveError = String(e);
		}
	}

	function startRename() {
		renameValue = collection;
		renameError = '';
		renaming = true;
	}

	async function handleRename(e: Event) {
		e.preventDefault();
		const newName = renameValue.trim();
		if (!newName || newName === collection) { renaming = false; return; }
		renameError = '';
		try {
			await app.withOp('Renaming collection', () => renameCollection(collection, newName));
			invalidateCache('collections');
			await app.loadCollections();
			renaming = false;
			goto(`/collection/${encodeURIComponent(newName)}`);
		} catch (e) {
			renameError = String(e);
		}
	}

	// Turns the collection into a deck, its cards landing on the first default board
	async function handleConvert() {
		confirmConvert = false;
		try {
			await app.withOp('Converting to deck', () => setCollectionKind(collection, 'deck'));
			invalidateCache('collections');
			await app.loadCollections();
			goto(`/deck/${encodeURIComponent(collection)}`);
		} catch (e) {
			console.error('[gathers] collection convert failed:', e);
		}
	}

	function pickImportFile(e: Event) {
		const input = e.target as HTMLInputElement;
		importFile = input.files?.[0] ?? null;
		// Lets the same file be picked again later.
		input.value = '';
	}
</script>

<div class="toolbar">
	<!-- Search toggle -->
	<button
		class="btn"
		class:btn-accent={searchOpen}
		onclick={onSearchOpen}
		title="Search & add cards"
	>
		<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
			<circle cx="6" cy="6" r="4.5" stroke="currentColor" stroke-width="1.5"/>
			<path d="M10 10l2.5 2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
		</svg>
		{searchOpen ? 'Close search' : 'Search & add'}
	</button>

	<!-- Filter toggle -->
	{#if onFilterOpen}
		<button
			class="btn"
			class:btn-accent={filterOpen || filterActive}
			onclick={onFilterOpen}
			title="Filter collection"
		>
			<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
				<path d="M1 3h12M3 7h8M5 11h4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
			</svg>
			Filter{filterActive ? ' ●' : ''}
		</button>
	{/if}

	<div class="toolbar-sep"></div>

	<!-- Selection -->
	<button class="btn btn-ghost btn-sm" onclick={() => app.selectAll(groups.map(g => g.id))}>
		Select all
	</button>
	{#if selectedList.length > 0}
		<button class="btn btn-ghost btn-sm" onclick={() => app.clearSelected()}>Clear selection</button>
	{/if}
	<div class="toolbar-sep"></div>

	{#if selectedList.length > 0}
		<span class="selection-badge">{selectedList.length} selected</span>

		<!-- Delete selected -->
		<button class="btn btn-danger" onclick={() => confirmDelete = 'cards'} title="Delete selected cards">
			<svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
				<path d="M2 4h10M5 4V3a1 1 0 011-1h2a1 1 0 011 1v1M11 4l-.9 8.1A1 1 0 019.1 13H4.9a1 1 0 01-1-.9L3 4" stroke="currentColor" stroke-width="1.2" fill="none"/>
			</svg>
			Delete selected
		</button>

		<!-- Move selected -->
		{#if otherCollections.length > 0}
			<div style="display:flex; gap:6px; align-items:center;">
				<select class="input" style="height:34px;padding:4px 28px 4px 10px;" bind:value={moveDest}>
					{#each otherCollections as col}
						<option value={col.id}>{col.id}</option>
					{/each}
				</select>
				<button class="btn" onclick={handleMove}>
					<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
						<path d="M3 7h8M9 5l2 2-2 2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
					Move
				</button>
				{#if moveError}<span style="color:var(--danger);font-size:0.78rem;">{moveError}</span>{/if}
			</div>
		{/if}

		<div class="toolbar-sep"></div>
	{/if}

	<!-- Import -->
	<label class="btn" title="Import cards from CSV/file">
		<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
			<path d="M7 9V2M4 7l3 3 3-3M2 11h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
		</svg>
		Import
		<input type="file" accept=".csv,.txt,text/csv" style="display:none;" onchange={pickImportFile} />
	</label>

	<!-- Export -->
	<button class="btn" title="Export collection as CSV" onclick={() => (exportOpen = true)}>
		<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
			<path d="M7 2v7M4 6l3-3 3 3M2 11h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
		</svg>
		Export
	</button>

	<div class="toolbar-sep"></div>

	<!-- Rename collection -->
	{#if renaming}
		<form style="display:flex; gap:6px; align-items:center;" onsubmit={handleRename}>
			<!-- svelte-ignore a11y_autofocus -->
			<input
				class="input"
				style="height:34px; padding:4px 10px; width:160px;"
				bind:value={renameValue}
				autofocus
			/>
			<button type="submit" class="btn btn-accent">Rename</button>
			<button type="button" class="btn btn-ghost" onclick={() => renaming = false}>✕</button>
			{#if renameError}<span style="color:var(--danger);font-size:0.78rem;">{renameError}</span>{/if}
		</form>
	{:else}
		<button class="btn" onclick={startRename} title="Rename collection">
			<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
				<path d="M2 10.5l1-3L9.5 1.5a1.41 1.41 0 012 2L5 10l-3 .5z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
				<path d="M8 3l2 2" stroke="currentColor" stroke-width="1.3"/>
			</svg>
			Rename
		</button>
	{/if}

	<!-- Convert to a deck: same cards, but gaining boards and a build state -->
	<button class="btn" onclick={() => confirmConvert = true} title="Make this a deck">
		Convert to deck
	</button>

	<!-- Share links -->
	{#if onShareOpen}
		<button class="btn" onclick={onShareOpen} title="Manage shareable links">
			<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
				<circle cx="3" cy="7" r="1.8" stroke="currentColor" stroke-width="1.3"/>
				<circle cx="11" cy="3" r="1.8" stroke="currentColor" stroke-width="1.3"/>
				<circle cx="11" cy="11" r="1.8" stroke="currentColor" stroke-width="1.3"/>
				<path d="M4.6 6.2L9.4 3.8M4.6 7.8L9.4 10.2" stroke="currentColor" stroke-width="1.3"/>
			</svg>
			Share
		</button>
	{/if}

	<!-- Delete collection — not offered for the default collection, which the server won't delete -->
	{#if removable}
		<button class="btn btn-danger" onclick={() => confirmDelete = 'collection'}>
			<svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
				<path d="M2 4h10M5 4V3a1 1 0 011-1h2a1 1 0 011 1v1M11 4l-.9 8.1A1 1 0 019.1 13H4.9a1 1 0 01-1-.9L3 4" stroke="currentColor" stroke-width="1.2" fill="none"/>
			</svg>
			Delete collection
		</button>
	{/if}

	<!-- Purchase history — rightmost -->
	{#if onHistoryOpen && app.pricingEnabled}
		<div class="toolbar-sep"></div>
		<button class="btn" onclick={onHistoryOpen}>
			<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
				<circle cx="7" cy="7" r="5.5" stroke="currentColor" stroke-width="1.3"/>
				<path d="M7 4v3.5l2 1.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
			</svg>
			Purchase history
		</button>
	{/if}
</div>

{#if importFile}
	<CsvImportModal
		{collection}
		file={importFile}
		onclose={() => (importFile = null)}
		ondone={() => { importFile = null; onRefresh(); }}
	/>
{/if}

{#if exportOpen}
	<CsvExportModal {collection} onclose={() => (exportOpen = false)} />
{/if}

<!-- Confirm dialogs -->
{#if confirmDelete === 'collection'}
	<ConfirmDialog
		title="Delete collection"
		message="Delete '{collection}' and all its card entries? This cannot be undone."
		confirmLabel="Delete"
		danger
		onconfirm={handleDeleteCollection}
		oncancel={() => confirmDelete = null}
	/>
{/if}

{#if confirmDelete === 'cards'}
	<ConfirmDialog
		title="Delete {selectedList.length} card{selectedList.length > 1 ? 's' : ''}"
		message="Remove the selected cards from '{collection}'?"
		confirmLabel="Delete"
		danger
		onconfirm={handleDeleteCards}
		oncancel={() => confirmDelete = null}
	/>
{/if}

{#if confirmConvert}
	<ConfirmDialog
		title="Convert to deck"
		message="Turn '{collection}' into a deck? Its cards move onto a Mainboard, and it gains boards and a build state. Prices and purchase history don't apply to a deck."
		confirmLabel="Convert"
		onconfirm={handleConvert}
		oncancel={() => confirmConvert = false}
	/>
{/if}
