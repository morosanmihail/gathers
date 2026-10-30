<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { app } from '$lib/state.svelte';
	import { getCollectionCount, getCollectionValue, getDeckCardCount } from '$lib/api';
	import { formatMoneyList, sumByCurrency, type Money } from '$lib/currency.svelte';
	import ConvertedTotal from '$lib/components/ConvertedTotal.svelte';
	import { isDeck } from '$lib/types';
	import type { CollectionKind } from '$lib/types';

	interface CollectionStats {
		id: string;
		kind: CollectionKind;
		count: number;
		values: Money[];
	}

	let stats = $state<CollectionStats[]>([]);
	let loading = $state(true);

	// Split rather than tallied together: a deck row is an allocation of a card that some
	// collection already owns, so counting both would count the same physical card twice. The
	// totals stay a statement about what's owned; decks get a separate tally.
	const collections = $derived(stats.filter((c) => !isDeck(c)));
	const decks = $derived(stats.filter((c) => isDeck(c)));

	const totalCards = $derived(collections.reduce((s, c) => s + c.count, 0));
	const totalValues = $derived(sumByCurrency(collections.flatMap(c => c.values)));
	const hasValue = (values: Money[]) => values.some(m => m.value > 0);

	onMount(async () => {
		// System info may already be loaded by the layout; if not, wait for it
		if (!app.ready) await app.loadSystemInfo();

		if (!app.collectionsEnabled) {
			goto('/search', { replaceState: true });
			return;
		}

		await app.loadCollections();
		const results = await Promise.all(
			app.collections.map(async (col) => {
				// A deck's cards aren't in `cards`, so the collection count and value endpoints
				// would both report zero for it. Count what the deck actually holds instead, and
				// skip pricing: a deck records what is to be played, not what was paid for
				if (isDeck(col)) {
					return {
						id: col.id,
						kind: col.kind,
						count: await getDeckCardCount(col.id),
						values: []
					};
				}
				const [count, val] = await Promise.all([
					getCollectionCount(col.id),
					app.pricingEnabled ? getCollectionValue(col.id) : Promise.resolve(null)
				]);
				const values = (val?.currencies ?? []).map(c => ({ currency: c.currency, value: c.total_value }));
				return { id: col.id, kind: col.kind, count, values };
			})
		);
		stats = results;
		loading = false;
	});
</script>

<svelte:head>
	<title>Home - gatheRs</title>
</svelte:head>

<div>
	{#if loading}
		<div class="loading-row"><div class="spinner"></div> Loading…</div>
	{:else}
		<div class="page-header">
			<h1 class="page-title" style="font-size: 1.6rem;">Collection Overview</h1>
		</div>

		<!-- Stats row -->
		<div class="stats-grid">
			<div class="stat-card">
				<div class="stat-label">Collections</div>
				<div class="stat-value">{collections.length}</div>
			</div>
			{#if decks.length > 0}
				<div class="stat-card">
					<div class="stat-label">Decks</div>
					<div class="stat-value">{decks.length}</div>
				</div>
			{/if}
			<div class="stat-card">
				<div class="stat-label">Total Cards</div>
				<div class="stat-value">{totalCards.toLocaleString()}</div>
			</div>
			{#if app.pricingEnabled && hasValue(totalValues)}
				<div class="stat-card">
					<div class="stat-label">Total Value</div>
					<div class="stat-value">{formatMoneyList(totalValues)} <ConvertedTotal items={totalValues} /></div>
					<div class="stat-sub">Estimated market value</div>
				</div>
			{/if}
		</div>

		<!-- Collection cards -->
		{#if stats.length === 0}
			<div class="empty-state" style="padding: 60px 20px;">
				<div class="empty-state-icon">📦</div>
				<div class="empty-state-text">No collections yet. Create one using the + New tab above.</div>
			</div>
		{:else}
			{#if collections.length > 0}
				<div style="padding: 8px 20px 4px;">
					<h2 style="font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text2);">Your Collections</h2>
				</div>
				<div class="collections-overview">
					{#each collections as col}
						<a href="/collection/{encodeURIComponent(col.id)}" class="collection-card">
							<div class="collection-card-name">{col.id}</div>
							<div class="collection-card-stats">
								<div class="collection-card-stat">
									<div class="collection-card-stat-val">{col.count.toLocaleString()}</div>
									<div class="collection-card-stat-lbl">Cards</div>
								</div>
								{#if app.pricingEnabled && hasValue(col.values)}
									<div class="collection-card-stat">
										<div class="collection-card-stat-val">{formatMoneyList(col.values)}</div>
										<div class="collection-card-stat-lbl">Value</div>
									</div>
								{/if}
							</div>
						</a>
					{/each}
				</div>
			{/if}

			{#if decks.length > 0}
				<div style="padding: 8px 20px 4px;">
					<h2 style="font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text2);">Your Decks</h2>
				</div>
				<div class="collections-overview">
					{#each decks as deck}
						<a href="/deck/{encodeURIComponent(deck.id)}" class="collection-card">
							<div class="collection-card-name">{deck.id}</div>
							<div class="collection-card-stats">
								<div class="collection-card-stat">
									<div class="collection-card-stat-val">{deck.count.toLocaleString()}</div>
									<div class="collection-card-stat-lbl">Cards</div>
								</div>
							</div>
						</a>
					{/each}
				</div>
			{/if}
		{/if}

		<div style="padding: 24px 20px 0; display: flex; gap: 12px; flex-wrap: wrap;">
			<a href="/search" class="btn btn-accent">
				<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
					<circle cx="6" cy="6" r="4.5" stroke="currentColor" stroke-width="1.5"/>
					<path d="M10 10l2.5 2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
				</svg>
				Search Cards
			</a>
			<a href="/random" class="btn">
				🎲 Random Card
			</a>
		</div>
	{/if}
</div>
