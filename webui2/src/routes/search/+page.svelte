<script lang="ts">
	import { page as appPage } from '$app/state';
	import { afterNavigate } from '$app/navigation';
	import { replaceState } from '$app/navigation';
	import SearchPanel from '$lib/components/SearchPanel.svelte';
	import CardTile from '$lib/components/CardTile.svelte';
	import CardRow from '$lib/components/CardRow.svelte';
	import Pagination from '$lib/components/Pagination.svelte';
	import CardDetailModal from '$lib/components/CardDetailModal.svelte';
	import { searchMtg, searchRiftbound, searchPokemon, addCardToCollection, getMtgPrices, getPokemonPrices, PAGE_SIZE } from '$lib/api';
	import { app } from '$lib/state.svelte';
	import { defaultFilters, bestPrice } from '$lib/types';
	import type { AnyCard, CollectionCard, CardPrices, SearchFilters } from '$lib/types';

	let filters = $state(defaultFilters());
	let results = $state<AnyCard[]>([]);
	let prices = $state<Record<string, CardPrices>>({});
	let loading = $state(false);
	let page = $state(1);
	let total = $state(0);
	let searched = $state(false);
	let activeSystem = $state('');
	let appliedQS = $state('');

	let addTarget = $state<AnyCard | null>(null);
	let addCollection = $state('');
	let addPrice = $state('');
	let toast = $state('');
	let detailCard = $state<AnyCard | null>(null);

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

	function applyParamsAndSearch(qs: string) {
		const params = new URLSearchParams(qs);
		const overrides: Partial<typeof filters> = {};
		let hasFilter = false;

		const str = (k: string) => params.get(k) ?? '';
		if (params.has('name'))            { overrides.name = str('name'); hasFilter = true; }
		if (params.has('set'))             { overrides.setCode = str('set'); hasFilter = true; }
		if (params.has('artist'))          { overrides.artist = str('artist'); hasFilter = true; }
		if (params.has('text'))            { overrides.text = str('text'); hasFilter = true; }
		if (params.has('rarity'))          { overrides.rarity = str('rarity'); hasFilter = true; }
		if (params.has('collectorNumber')) { overrides.collectorNumber = str('collectorNumber'); hasFilter = true; }
		if (params.has('colors'))          { overrides.colorIdentities = str('colors').split(',').filter(Boolean); hasFilter = true; }
		if (params.has('sortBy'))          overrides.sortBy = str('sortBy');
		if (params.has('sortOrder'))       overrides.sortOrder = str('sortOrder') as 'Asc' | 'Desc';
		if (params.has('system'))          activeSystem = str('system');

		// Advanced (MTG-only) filters
		if (params.has('manaMin'))         { overrides.manaValueMin = str('manaMin'); hasFilter = true; }
		if (params.has('manaMax'))         { overrides.manaValueMax = str('manaMax'); hasFilter = true; }
		if (params.has('cardColors'))      { overrides.colors = str('cardColors').split(',').filter(Boolean); hasFilter = true; }
		if (params.has('keywords'))        { overrides.keywords = str('keywords'); hasFilter = true; }
		if (params.has('power'))           { overrides.power = str('power'); hasFilter = true; }
		if (params.has('toughness'))       { overrides.toughness = str('toughness'); hasFilter = true; }
		if (params.has('loyalty'))         { overrides.loyalty = str('loyalty'); hasFilter = true; }
		if (params.has('defense'))         { overrides.defense = str('defense'); hasFilter = true; }
		if (params.has('reserved'))        { overrides.isReserved = str('reserved') as SearchFilters['isReserved']; hasFilter = true; }
		if (params.has('promo'))           { overrides.isPromo = str('promo') as SearchFilters['isPromo']; hasFilter = true; }
		if (params.has('reprint'))         { overrides.isReprint = str('reprint') as SearchFilters['isReprint']; hasFilter = true; }
		if (params.has('fullArt'))         { overrides.isFullArt = str('fullArt') as SearchFilters['isFullArt']; hasFilter = true; }
		if (params.has('borderColor'))     { overrides.borderColor = str('borderColor'); hasFilter = true; }
		if (params.has('legalIn'))         { overrides.legalIn = str('legalIn'); hasFilter = true; }

		if (hasFilter) {
			filters = { ...defaultFilters(), ...overrides };
			const p = params.has('page') ? Math.max(1, parseInt(params.get('page')!) || 1) : 1;
			doSearch(p);
		}
	}

	// afterNavigate fires once the router has finished initializing — for the
	// initial load, and again for any later navigation to /search?... while
	// already on this route (SvelteKit reuses the component, so onMount alone
	// would only ever catch the first load; replaceState throws if called
	// before the router is ready, which rules out a plain $effect/onMount).
	afterNavigate(() => {
		const qs = appPage.url.search;
		if (qs === appliedQS) return;
		appliedQS = qs;
		applyParamsAndSearch(qs);
	});

	function handleSystemChange(sys: string) {
		activeSystem = sys;
		// Reset results when switching system
		results = [];
		prices = {};
		searched = false;
		filters = defaultFilters();
	}

	function buildSearchUrl(p: number): string {
		const params = new URLSearchParams();
		if (filters.name)                    params.set('name', filters.name);
		if (filters.setCode)                 params.set('set', filters.setCode);
		if (filters.artist)                  params.set('artist', filters.artist);
		if (filters.text)                    params.set('text', filters.text);
		if (filters.rarity)                  params.set('rarity', filters.rarity);
		if (filters.collectorNumber)         params.set('collectorNumber', filters.collectorNumber);
		if (filters.colorIdentities.length)  params.set('colors', filters.colorIdentities.join(','));
		if (filters.sortBy !== 'Name')       params.set('sortBy', filters.sortBy);
		if (filters.sortOrder !== 'Asc')     params.set('sortOrder', filters.sortOrder);
		if (activeSystem)                    params.set('system', activeSystem);

		// Advanced (MTG-only) filters
		if (filters.manaValueMin)            params.set('manaMin', filters.manaValueMin);
		if (filters.manaValueMax)            params.set('manaMax', filters.manaValueMax);
		if (filters.colors.length)           params.set('cardColors', filters.colors.join(','));
		if (filters.keywords)                params.set('keywords', filters.keywords);
		if (filters.power)                   params.set('power', filters.power);
		if (filters.toughness)               params.set('toughness', filters.toughness);
		if (filters.loyalty)                 params.set('loyalty', filters.loyalty);
		if (filters.defense)                 params.set('defense', filters.defense);
		if (filters.isReserved)              params.set('reserved', filters.isReserved);
		if (filters.isPromo)                 params.set('promo', filters.isPromo);
		if (filters.isReprint)               params.set('reprint', filters.isReprint);
		if (filters.isFullArt)               params.set('fullArt', filters.isFullArt);
		if (filters.borderColor)             params.set('borderColor', filters.borderColor);
		if (filters.legalIn)                 params.set('legalIn', filters.legalIn);
		if (p > 1)                           params.set('page', String(p));
		const qs = params.toString();
		return qs ? `?${qs}` : '/search';
	}

	async function doSearch(p = 1) {
		const url = buildSearchUrl(p);
		const qIdx = url.indexOf('?');
		appliedQS = qIdx >= 0 ? url.slice(qIdx) : '';
		replaceState(url, {});
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
			// The search endpoints don't return a total count, so page count is a moving
			// estimate: exact once a short page proves we've hit the end, otherwise "at
			// least one more page" so the pager grows as the user pages forward instead
			// of jumping straight to a bogus 99. If we overshot (e.g. jumped straight to
			// a stale/URL-provided page number past the real end), step back rather than
			// showing a dead empty page with no way back except browser back.
			if (data.length === 0 && p > 1) {
				await doSearch(p - 1);
				return;
			}
			results = data;
			total = data.length < PAGE_SIZE ? (p - 1) * PAGE_SIZE + data.length : p * PAGE_SIZE + 1;
			searched = true;

			if (app.pricingEnabled && activeSystem !== 'RiftboundSQLite' && data.length) {
				const ids = data.map(c => c.id);
				const fetchPrices = activeSystem === 'PokemonSQLite' ? getPokemonPrices : getMtgPrices;
				fetchPrices(ids).then(result => { prices = { ...prices, ...result }; });
			}
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
								price={bestPrice(prices[card.id])}
								cardPrices={prices[card.id]}
								onAdd={app.collectionsEnabled ? promptAdd : undefined}
								onclick={(c) => detailCard = c as AnyCard}
							/>
						{/each}
					</div>
				{:else}
					<div class="card-list card-list-results">
						<div class="card-list-header">
							<div class="card-list-col"></div>
							<div class="card-list-col">Name</div>
							<div class="card-list-col">Set</div>
							<div class="card-list-col">Rarity</div>
							<div class="card-list-col">Price</div>
							<div class="card-list-col"></div>
						</div>
						{#each results as card (card.id)}
							<CardRow
								{card}
								price={bestPrice(prices[card.id])}
								cardPrices={prices[card.id]}
								onAdd={app.collectionsEnabled ? promptAdd : undefined}
								onclick={(c) => detailCard = c as AnyCard}
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

{#if detailCard}
	<CardDetailModal card={detailCard} onclose={() => detailCard = null} />
{/if}
