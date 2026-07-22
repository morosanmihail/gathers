<script lang="ts">
	import SearchPanel from './SearchPanel.svelte';
	import CardTile from './CardTile.svelte';
	import CardRow from './CardRow.svelte';
	import Pagination from './Pagination.svelte';
	import CardDetailModal from './CardDetailModal.svelte';
	import { searchMtg, searchRiftbound, searchPokemon, addCardToCollection, getMtgPrices, getPokemonPrices, PAGE_SIZE } from '$lib/api';
	import { app } from '$lib/state.svelte';
	import { defaultFilters, bestPrice } from '$lib/types';
	import type { AnyCard, CollectionCard, CardPrices, ViewMode } from '$lib/types';

	interface Props {
		collection: string;
		onclose: () => void;
		onAdded?: () => void;
	}

	let { collection, onclose, onAdded }: Props = $props();

	let filters = $state(defaultFilters());
	let results = $state<AnyCard[]>([]);
	let loading = $state(false);
	let page = $state(1);
	let total = $state(0);
	let searched = $state(false);
	let toast = $state('');
	let addPrice = $state('');
	let activeSystem = $state('');
	let viewMode = $state<ViewMode>('grid');
	let prices = $state<Record<string, CardPrices>>({});
	let detailCard = $state<AnyCard | null>(null);

	$effect(() => {
		if (!activeSystem && app.systems.length > 0) activeSystem = app.systems[0];
	});

	function handleSystemChange(sys: string) {
		activeSystem = sys;
		results = [];
		prices = {};
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
			// No total-count endpoint for these searches: grow the estimate as the user
			// pages forward (exact once a short page proves the end), and step back if
			// we overshot instead of stranding them on a dead empty page.
			if (data.length === 0 && p > 1) {
				await doSearch(p - 1);
				return;
			}
			results = data;
			total = data.length < PAGE_SIZE ? (p - 1) * PAGE_SIZE + data.length : p * PAGE_SIZE + 1;
			searched = true;

			if (app.pricingEnabled) {
				const ids = data.map(c => c.id);
				const fetch = activeSystem === 'PokemonSQLite' ? getPokemonPrices
					: activeSystem === 'RiftboundSQLite' ? null
					: getMtgPrices;
				fetch?.(ids).then(result => { prices = { ...prices, ...result }; });
			}
		} finally {
			loading = false;
		}
	}

	async function addCard(card: AnyCard | CollectionCard, foil = false) {
		const price = addPrice !== '' ? parseFloat(addPrice) : null;
		const purchasePrice = price != null && isFinite(price) && price > 0 ? price : null;
		addPrice = '';
		try {
			await app.withOp(`Adding ${card.name}`, () =>
				addCardToCollection(collection, card.id, foil ? 0 : 1, foil ? 1 : 0, purchasePrice)
			);
			toast = `Added ${card.name}${foil ? ' (foil)' : ''}`;
			setTimeout(() => toast = '', 2000);
			onAdded?.();
		} catch {
			toast = 'Failed to add card';
			setTimeout(() => toast = '', 2000);
		}
	}

	function onOverlayClick(e: MouseEvent) {
		if ((e.target as HTMLElement).classList.contains('modal-overlay')) onclose();
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="modal-overlay" onclick={onOverlayClick} onkeydown={(e) => e.key === 'Escape' && onclose()} role="dialog" aria-modal="true" tabindex="-1">
	<div class="modal">
		<div class="modal-header">
			<h3>Search & Add to "{collection}"</h3>
				<div class="view-toggle" title="Toggle view">
					<button
						class="view-toggle-btn"
						class:active={viewMode === 'grid'}
						onclick={() => viewMode = 'grid'}
						title="Grid view"
					>
						<svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
							<rect x="0" y="0" width="6" height="6" rx="1"/>
							<rect x="8" y="0" width="6" height="6" rx="1"/>
							<rect x="0" y="8" width="6" height="6" rx="1"/>
							<rect x="8" y="8" width="6" height="6" rx="1"/>
						</svg>
					</button>
					<button
						class="view-toggle-btn"
						class:active={viewMode === 'list'}
						onclick={() => viewMode = 'list'}
						title="List view"
					>
						<svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
							<rect x="0" y="0" width="14" height="2" rx="1"/>
							<rect x="0" y="4" width="14" height="2" rx="1"/>
							<rect x="0" y="8" width="14" height="2" rx="1"/>
							<rect x="0" y="12" width="14" height="2" rx="1"/>
						</svg>
					</button>
				</div>
			<button class="btn btn-ghost btn-icon" onclick={onclose} title="Close">✕</button>
		</div>
		<!-- Purchase price bar -->
		<div style="padding: 8px 20px; border-bottom: 1px solid var(--border); background: var(--surface); display:flex; align-items:center; gap:8px;">
			<span style="font-size:0.82rem; color:var(--text2);">Purchase price for next add:</span>
			<span style="color:var(--text2);">$</span>
			<input
				type="number" min="0" step="0.01" placeholder="optional"
				class="input" style="width:110px; height:28px; padding:3px 8px; font-family:'JetBrains Mono',monospace; font-size:0.82rem;"
				bind:value={addPrice}
			/>
			{#if addPrice}
				<button class="btn btn-ghost btn-sm" onclick={() => addPrice = ''}>Clear</button>
			{/if}
		</div>
		<div class="modal-body" style="display: grid; grid-template-columns: 280px 1fr; gap: 20px; align-items: start;">
			<SearchPanel
				{filters}
				onfilters={(f) => filters = f}
				onsubmit={() => doSearch(1)}
				systems={app.systems}
				{activeSystem}
				onSystemChange={handleSystemChange}
				compact
			/>

			<div>
				{#if toast}
					<div class="toast success" style="margin-bottom: 12px;">{toast}</div>
				{/if}

				{#if loading}
					<div class="loading-row"><div class="spinner"></div> Searching…</div>
				{:else if !searched}
					<div class="empty-state">
						<div class="empty-state-icon">🔍</div>
						<div class="empty-state-text">Enter search terms and press Search</div>
					</div>
				{:else if results.length === 0}
					<div class="empty-state">
						<div class="empty-state-icon">📭</div>
						<div class="empty-state-text">No cards found</div>
					</div>
				{:else}
					{#if viewMode === 'grid'}
						<div class="card-grid" style="padding: 0; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 10px;">
							{#each results as card (card.id)}
								<CardTile {card} {collection} price={bestPrice(prices[card.id])} cardPrices={prices[card.id]} onAdd={(c) => addCard(c)} onAddFoil={(c) => addCard(c, true)} onclick={(c) => detailCard = c as AnyCard} />
							{/each}
						</div>
					{:else}
						<div class="card-list card-list-search">
							<div class="card-list-header">
								<div class="card-list-col"></div>
								<div class="card-list-col">Set</div>
								<div class="card-list-col">Name</div>
								<div class="card-list-col">R</div>
								<div class="card-list-col">Price</div>
								<div class="card-list-col"></div>
							</div>
							{#each results as card (card.id)}
								<CardRow {card} {collection} price={bestPrice(prices[card.id])} cardPrices={prices[card.id]} onAdd={(c) => addCard(c)} onAddFoil={(c) => addCard(c, true)} onclick={(c) => detailCard = c as AnyCard} />
							{/each}
						</div>
					{/if}
					<Pagination {total} {page} onchange={(p) => doSearch(p)} />
				{/if}
			</div>
		</div>
	</div>
</div>

{#if detailCard}
	<CardDetailModal card={detailCard} onclose={() => detailCard = null} />
{/if}
