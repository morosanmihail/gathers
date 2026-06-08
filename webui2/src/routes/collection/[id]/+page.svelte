<script lang="ts">
	import { page } from '$app/stores';
	import CardTile from '$lib/components/CardTile.svelte';
	import CardRow from '$lib/components/CardRow.svelte';
	import Pagination from '$lib/components/Pagination.svelte';
	import CollectionToolbar from '$lib/components/CollectionToolbar.svelte';
	import SearchModal from '$lib/components/SearchModal.svelte';
	import PurchaseHistoryModal from '$lib/components/PurchaseHistoryModal.svelte';
	import {
		getCollectionCards, getCollectionCount,
		searchCollectionCards, searchCollectionCount,
		addCardToCollection, deleteCardFromCollection,
		getMtgPrices, getPokemonPrices, getCollectionValue, PAGE_SIZE
	} from '$lib/api';
	import { app } from '$lib/state.svelte';
	import { goto } from '$app/navigation';
	import { defaultFilters } from '$lib/types';
	import type { CollectionCard, CardPrices, ValueBreakdown } from '$lib/types';

	const collectionId = $derived(decodeURIComponent($page.params.id ?? ''));

	let cards = $state<CollectionCard[]>([]);
	let total = $state(0);
	let currentPage = $state(1);
	let loading = $state(true);
	let refreshKey = $state(0);
	let prices = $state<Record<string, CardPrices>>({});
	let collectionValue = $state<number | null>(null);
	let valueBreakdown = $state<ValueBreakdown | null>(null);
	let valueHover = $state(false);

	// Fields the /list endpoint accepts; everything else goes through /search
	const COLLECTION_SORT_FIELDS = new Set(['TimeAdded', 'Quantity', 'FoilQuantity', 'Provider']);

	let filterActive = $state(false);
	let collectionFilters = $state(defaultFilters());
	let sortBy = $state('');
	let sortOrder = $state<'Asc' | 'Desc'>('Asc');
	let searchOpen = $state(false);
	let historyOpen = $state(false);

	const sortIsCardLevel = $derived(sortBy !== '' && !COLLECTION_SORT_FIELDS.has(sortBy));

	async function load(p = currentPage) {
		loading = true;
		try {
			if (filterActive || sortIsCardLevel) {
				// Card-level sort or active filter: use search endpoint
				const filtersWithSort = {
					...collectionFilters,
					sortBy: sortBy || collectionFilters.sortBy,
					sortOrder
				};
				const [data, count] = await Promise.all([
					searchCollectionCards(collectionId, filtersWithSort, p),
					searchCollectionCount(collectionId, filtersWithSort)
				]);
				cards = data;
				total = count;
			} else {
				const [data, count] = await Promise.all([
					getCollectionCards(collectionId, p, sortBy, sortOrder),
					getCollectionCount(collectionId)
				]);
				cards = data;
				total = count;
			}

			// Fetch prices + value in parallel (non-blocking, results are cached)
			if (app.pricingEnabled) {
				const mtgIds = cards
					.filter(c => !c.provider || c.provider === 'MagicSQLite' || c.provider === 'Scryfall')
					.map(c => c.id);
				const pokemonIds = cards
					.filter(c => c.provider === 'PokemonSQLite')
					.map(c => c.id);
				const fetches: Promise<Record<string, CardPrices>>[] = [];
				if (mtgIds.length) fetches.push(getMtgPrices(mtgIds));
				if (pokemonIds.length) fetches.push(getPokemonPrices(pokemonIds));
				const valuePromise = getCollectionValue(collectionId);
				if (fetches.length) {
					Promise.all(fetches).then(results => {
						prices = { ...prices, ...Object.assign({}, ...results) };
					});
				}
				valuePromise.then(v => { collectionValue = v.total_value ?? null; valueBreakdown = v; });
			}
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		if (app.ready && !app.collectionsEnabled) {
			goto('/search', { replaceState: true });
			return;
		}
		collectionId; refreshKey;
		currentPage = 1;
		app.clearSelected();
		load(1);
	});

	function refreshValue() {
		if (app.pricingEnabled) {
			getCollectionValue(collectionId).then(v => { collectionValue = v.total_value ?? null; valueBreakdown = v; });
		}
	}

	async function adjustCardQty(card: CollectionCard, delta: number, foil: boolean, purchasePrice?: number | null) {
		try {
			if (delta > 0) {
				await addCardToCollection(collectionId, card.id, foil ? 0 : 1, foil ? 1 : 0, purchasePrice);
			} else {
				await deleteCardFromCollection(collectionId, card.id, foil ? 0 : 1, foil ? 1 : 0);
			}
			// Optimistically update local state
			cards = cards.map(c => {
				if (c.id !== card.id) return c;
				const qty = foil ? c.quantity : Math.max(0, c.quantity + delta);
				const foilQty = foil ? Math.max(0, c.foilQuantity + delta) : c.foilQuantity;
				return { ...c, quantity: qty, foilQuantity: foilQty };
			}).filter(c => c.quantity > 0 || c.foilQuantity > 0);
			total = Math.max(0, total + delta);
			refreshValue();
		} catch (e) {
			console.error('[gathers] adjustCardQty failed:', e);
		}
	}

	function handleRefresh() {
		refreshKey++;
		refreshValue();
	}

	function handlePageChange(p: number) {
		currentPage = p;
		load(p);
		window.scrollTo({ top: 0, behavior: 'smooth' });
	}

	function handleSortClick(field: string) {
		if (sortBy === field) {
			sortOrder = sortOrder === 'Asc' ? 'Desc' : 'Asc';
		} else {
			sortBy = field;
			sortOrder = 'Asc';
		}
		currentPage = 1;
		load(1);
	}

	function bestPrice(cardPrices: CardPrices): string | null {
		if (!cardPrices?.paper) return null;
		const vals = Object.values(cardPrices.paper).flatMap(r => [r.normal, r.foil].filter(v => v != null)) as number[];
		if (!vals.length) return null;
		return `$${Math.min(...vals).toFixed(2)}`;
	}

	const listHeaders = [
		{ field: '',           label: '' },
		{ field: 'Name',       label: 'Name' },
		{ field: 'SetCode',    label: 'Set' },
		{ field: 'Rarity',     label: 'Rarity' },
		{ field: 'Artist',     label: 'Artist' },
		{ field: '',           label: 'Price' },
		{ field: 'Quantity',   label: 'Qty' },
		{ field: 'FoilQuantity', label: 'Foil' },
	];
</script>

<svelte:head>
	<title>{collectionId} - gatheRs</title>
</svelte:head>

<div>
	<CollectionToolbar
		collection={collectionId}
		{cards}
		onRefresh={handleRefresh}
		onSearchOpen={() => searchOpen = !searchOpen}
		{searchOpen}
		onHistoryOpen={() => historyOpen = true}
	/>

	<div class="page-header" style="padding-bottom: 8px;">
		<h1 class="page-title">{collectionId}</h1>
		{#if !loading}
			<span class="page-subtitle">{total.toLocaleString()} card{total !== 1 ? 's' : ''}</span>
		{/if}
		{#if collectionValue != null}
			<span
				class="page-subtitle"
				style="color: var(--accent-text); margin-left: auto; position: relative; cursor: default;"
				onmouseenter={() => valueHover = true}
				onmouseleave={() => valueHover = false}
			>
				≈ ${collectionValue.toFixed(2)}
				{#if valueHover && valueBreakdown}
					<div class="value-breakdown-tooltip">
						{#if valueBreakdown.profit != null}
							<div class="vb-row">
								<span class="vb-label">Profit</span>
								<span class="vb-val" style="color: {valueBreakdown.profit >= 0 ? 'var(--success)' : 'var(--danger)'}">
									{valueBreakdown.profit >= 0 ? '+' : ''}${valueBreakdown.profit.toFixed(2)}
								</span>
							</div>
						{/if}
						{#if valueBreakdown.untracked_value != null && valueBreakdown.untracked_value > 0}
							<div class="vb-row">
								<span class="vb-label">No purchase data</span>
								<span class="vb-val">≈ ${valueBreakdown.untracked_value.toFixed(2)}</span>
							</div>
						{/if}
						{#if valueBreakdown.priced_count != null && valueBreakdown.total_count != null}
							<div class="vb-row" style="margin-top: 6px; border-top: 1px solid var(--border); padding-top: 6px;">
								<span class="vb-label">Priced</span>
								<span class="vb-val">{valueBreakdown.priced_count} / {valueBreakdown.total_count}</span>
							</div>
						{/if}
					</div>
				{/if}
			</span>
		{/if}
	</div>

	{#if loading && cards.length === 0}
		<div class="loading-row"><div class="spinner"></div> Loading…</div>
	{:else if !loading && cards.length === 0}
		<div class="empty-state">
			<div class="empty-state-icon">📦</div>
			<div class="empty-state-text">No cards in this collection. Use Search & add above.</div>
		</div>
	{:else}
		{#if app.viewMode === 'grid'}
			<div class="card-grid">
				{#each cards as card (card.collectionId + '-' + card.id)}
					<CardTile {card} collectionMode collection={collectionId} price={bestPrice(prices[card.id])} cardPrices={prices[card.id]} onAdjust={adjustCardQty} />
				{/each}
			</div>
		{:else}
			<div class="card-list">
				<div class="card-list-header">
					{#each listHeaders as h}
						<div
							class="card-list-col"
							class:active={h.field !== '' && sortBy === h.field}
							role={h.field ? 'button' : undefined}
							tabindex={h.field ? 0 : undefined}
							onclick={() => h.field && handleSortClick(h.field)}
							onkeydown={(e) => e.key === 'Enter' && h.field && handleSortClick(h.field)}
						>
							{h.label}
							{#if h.field && sortBy === h.field}
								{sortOrder === 'Asc' ? ' ↑' : ' ↓'}
							{/if}
						</div>
					{/each}
				</div>
				{#each cards as card (card.collectionId + '-' + card.id)}
					<CardRow {card} collectionMode collection={collectionId} price={bestPrice(prices[card.id])} cardPrices={prices[card.id]} onAdjust={adjustCardQty} />
				{/each}
			</div>
		{/if}

		{#if loading}
			<div class="loading-row"><div class="spinner"></div></div>
		{/if}

		<Pagination {total} page={currentPage} onchange={handlePageChange} />
	{/if}
</div>

{#if searchOpen}
	<SearchModal
		collection={collectionId}
		onclose={() => searchOpen = false}
		onAdded={handleRefresh}
	/>
{/if}

{#if historyOpen}
	<PurchaseHistoryModal
		collection={collectionId}
		onclose={() => historyOpen = false}
	/>
{/if}
