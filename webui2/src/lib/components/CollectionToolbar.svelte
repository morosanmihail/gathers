<script lang="ts">
	import ConfirmDialog from './ConfirmDialog.svelte';
	import { app } from '$lib/state.svelte';
	import { goto } from '$app/navigation';
	import {
		deleteCollection,
		deleteCardFromCollection,
		moveCards,
		importCards,
		exportCollectionUrl,
		invalidateCache
	} from '$lib/api';
	import type { CollectionCard } from '$lib/types';

	interface Props {
		collection: string;
		cards: CollectionCard[];
		onRefresh: () => void;
		onSearchOpen: () => void;
		searchOpen: boolean;
		onHistoryOpen?: () => void;
	}

	let { collection, cards, onRefresh, onSearchOpen, searchOpen, onHistoryOpen }: Props = $props();

	let confirmDelete = $state<'collection' | 'cards' | null>(null);
	let moveDest = $state('');
	let moveError = $state('');
	let importing = $state(false);
	let importError = $state('');

	const selectedList = $derived(
		cards.filter(c => app.selectedCards.has(c.id))
	);

	const otherCollections = $derived(
		app.collections.filter(c => c.id !== collection)
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
				for (const card of selectedList) {
					await deleteCardFromCollection(collection, card.id, card.quantity, card.foilQuantity);
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
			await app.withOp(`Moving to ${moveDest}`, () =>
				moveCards(collection, moveDest, selectedList.map(c => ({
					id: c.id,
					quantity: c.quantity,
					foilQuantity: c.foilQuantity
				})))
			);
			app.clearSelected();
			onRefresh();
		} catch (e) {
			moveError = String(e);
		}
	}

	async function handleImport(e: Event) {
		const file = (e.target as HTMLInputElement).files?.[0];
		if (!file) return;
		importing = true;
		importError = '';
		try {
			await app.withOp('Importing', () => importCards(collection, file));
			onRefresh();
		} catch (err) {
			importError = String(err);
		} finally {
			importing = false;
			(e.target as HTMLInputElement).value = '';
		}
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

	<div class="toolbar-sep"></div>

	<!-- Selection info -->
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

		<button class="btn btn-ghost" onclick={() => app.clearSelected()}>Clear selection</button>
		<div class="toolbar-sep"></div>
	{:else}
		<button class="btn btn-ghost btn-sm" onclick={() => app.selectAll(cards.map(c => c.id))}>
			Select all
		</button>
		<div class="toolbar-sep"></div>
	{/if}

	<!-- Import -->
	<label class="btn" title="Import cards from CSV/file">
		<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
			<path d="M7 9V2M4 7l3 3 3-3M2 11h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
		</svg>
		{importing ? 'Importing…' : 'Import'}
		<input type="file" accept=".csv,.txt,.json" style="display:none;" onchange={handleImport} disabled={importing} />
	</label>
	{#if importError}<span style="color:var(--danger);font-size:0.78rem;">{importError}</span>{/if}

	<!-- Export -->
	<a href={exportCollectionUrl(collection)} class="btn" title="Export collection" download>
		<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
			<path d="M7 2v7M4 6l3-3 3 3M2 11h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
		</svg>
		Export
	</a>

	<div class="toolbar-sep"></div>

	<!-- Delete collection -->
	<button class="btn btn-danger" onclick={() => confirmDelete = 'collection'}>
		<svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
			<path d="M2 4h10M5 4V3a1 1 0 011-1h2a1 1 0 011 1v1M11 4l-.9 8.1A1 1 0 019.1 13H4.9a1 1 0 01-1-.9L3 4" stroke="currentColor" stroke-width="1.2" fill="none"/>
		</svg>
		Delete collection
	</button>

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
