<script lang="ts">
	import { onMount } from 'svelte';
	import SearchPanel from '$lib/components/SearchPanel.svelte';
	import CardTile from '$lib/components/CardTile.svelte';
	import CardRow from '$lib/components/CardRow.svelte';
	import Pagination from '$lib/components/Pagination.svelte';
	import { searchMtg, searchRiftbound, searchPokemon, addCardToCollection, PAGE_SIZE } from '$lib/api';
	import { app } from '$lib/state.svelte';
	import { defaultFilters } from '$lib/types';
	import type { AnyCard, CollectionCard } from '$lib/types';

	let filters = $state(defaultFilters());
	let results = $state<AnyCard[]>([]);
	let loading = $state(false);
	let page = $state(1);
	let total = $state(0);
	let searched = $state(false);
	let activeSystem = $state('');

	let addTarget = $state<AnyCard | null>(null);
	let addCollection = $state('');
	let addPrice = $state('');
	let toast = $state('');

	// Pick first available system once loaded
	$effect(() => {
		if (!activeSystem && app.systems.length > 0) {
			activeSystem = app.systems[0];
		}
	});

	$effect(() => {
		if (app.collections.length > 0 && !addCollection) {
			addCollection = app.collections[0].id;
		}
	});

	function handleSystemChange(sys: string) {
		activeSystem = sys;
		// Reset results when switching system
		results = [];
		searched = false;
		filters = defaultFilters();
	}

	async function doSearch(p = 1) {
		loading = true;
		page = p;
		try {
			let data: AnyCard[];
			if (activeSystem === 'RiftboundSQLite') {
				data = await searchRiftbound(filters, p);
			} else if (activeSystem === 'PokemonSQLite') {
				data = await searchPokemon(filters, p);
			} else {
				data = await searchMtg(filters, p);
			}
			results = data;
			if (p === 1) total = data.length >= PAGE_SIZE ? PAGE_SIZE * 99 : data.length;
			searched = true;
		} catch (e) {
			console.error(e);
		} finally {
			loading = false;
		}
	}

	function promptAdd(card: AnyCard | CollectionCard) {
		if (!app.collectionsEnabled) return;
		addTarget = card as AnyCard;
		addPrice = '';
	}

	async function confirmAdd() {
		if (!addTarget || !addCollection) return;
		const price = addPrice !== '' ? parseFloat(addPrice) : null;
		const purchasePrice = price != null && isFinite(price) && price > 0 ? price : null;
		try {
			await app.withOp(`Adding ${addTarget.name}`, () =>
				addCardToCollection(addCollection, addTarget!.id, 1, 0, purchasePrice)
			);
			toast = `Added "${addTarget.name}" to ${addCollection}`;
			setTimeout(() => toast = '', 3000);
		} catch (e) {
			toast = `Error: ${e}`;
			setTimeout(() => toast = '', 3000);
		} finally {
			addTarget = null;
		}
	}
</script>

<svelte:head>
	<title>Search - gatheRs</title>
</svelte:head>

<div>
	<div style="display: grid; grid-template-columns: 280px 1fr; gap: 0; align-items: start; min-height: calc(100vh - 94px);">
		<!-- Left: search form -->
		<div style="padding: 20px; border-right: 1px solid var(--border); position: sticky; top: 94px; max-height: calc(100vh - 94px); overflow-y: auto;">
			<SearchPanel
				{filters}
				onfilters={(f) => filters = f}
				onsubmit={() => doSearch(1)}
				systems={app.systems}
				{activeSystem}
				onSystemChange={handleSystemChange}
			/>
		</div>

		<!-- Right: results -->
		<div>
			{#if toast}
				<div class="toast-stack">
					<div class="toast success">{toast}</div>
				</div>
			{/if}

			{#if loading}
				<div class="loading-row"><div class="spinner"></div> Searching…</div>
			{:else if !searched}
				<div class="empty-state" style="padding: 80px 20px;">
					<div class="empty-state-icon">🔍</div>
					<div class="empty-state-text">Enter search terms and press Search</div>
				</div>
			{:else if results.length === 0}
				<div class="empty-state" style="padding: 80px 20px;">
					<div class="empty-state-icon">📭</div>
					<div class="empty-state-text">No cards found. Try adjusting your search.</div>
				</div>
			{:else}
				<div style="display:flex;align-items:center;justify-content:space-between;padding: 12px 20px 4px;">
					<span style="font-size:0.8rem;color:var(--text2);">{results.length} result{results.length !== 1 ? 's' : ''}</span>
				</div>

				{#if app.viewMode === 'grid'}
					<div class="card-grid">
						{#each results as card (card.id)}
							<CardTile
								{card}
								onAdd={app.collectionsEnabled ? promptAdd : undefined}
							/>
						{/each}
					</div>
				{:else}
					<div class="card-list">
						<div class="card-list-header">
							<div class="card-list-col"></div>
							<div class="card-list-col">Name</div>
							<div class="card-list-col">Set</div>
							<div class="card-list-col">Rarity</div>
							<div class="card-list-col">Artist</div>
							<div class="card-list-col">Text</div>
							<div class="card-list-col"></div>
							<div class="card-list-col"></div>
						</div>
						{#each results as card (card.id)}
							<CardRow
								{card}
								onAdd={app.collectionsEnabled ? promptAdd : undefined}
							/>
						{/each}
					</div>
				{/if}
				<Pagination {total} {page} onchange={(p) => doSearch(p)} />
			{/if}
		</div>
	</div>
</div>

<!-- Add to collection dialog -->
{#if addTarget}
	<div class="confirm-overlay" role="dialog" aria-modal="true">
		<div class="confirm-box">
			<h4>Add to collection</h4>
			<p>Add <strong>{addTarget.name}</strong> to:</p>
			{#if app.collections.length > 0}
				<select class="input" bind:value={addCollection} style="margin-bottom: 10px;">
					{#each app.collections as col}
						<option value={col.id}>{col.id}</option>
					{/each}
				</select>
				<div style="display:flex; align-items:center; gap:6px; margin-bottom:16px;">
					<span style="color:var(--text2); font-size:0.85rem;">Purchase price:</span>
					<span style="color:var(--text2);">$</span>
					<input
						type="number" min="0" step="0.01" placeholder="optional"
						class="input" style="width:110px; height:32px; padding:4px 8px; font-family:'JetBrains Mono',monospace;"
						bind:value={addPrice}
					/>
				</div>
			{:else}
				<p style="color: var(--danger); font-size: 0.85rem;">No collections yet. Create one first.</p>
			{/if}
			<div class="confirm-actions">
				<button class="btn" onclick={() => addTarget = null}>Cancel</button>
				<button class="btn btn-accent" onclick={confirmAdd} disabled={!app.collections.length}>Add</button>
			</div>
		</div>
	</div>
{/if}
