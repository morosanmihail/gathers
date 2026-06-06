<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { app } from '$lib/state.svelte';
	import { getCollectionCount, getCollectionValue } from '$lib/api';

	interface CollectionStats {
		id: string;
		count: number;
		value?: number;
	}

	let stats = $state<CollectionStats[]>([]);
	let loading = $state(true);

	const totalCards = $derived(stats.reduce((s, c) => s + c.count, 0));
	const totalValue = $derived(stats.reduce((s, c) => s + (c.value ?? 0), 0));

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
				const [count, val] = await Promise.all([
					getCollectionCount(col.id),
					app.pricingEnabled ? getCollectionValue(col.id) : Promise.resolve(null)
				]);
				return { id: col.id, count, value: val?.total_value as number | undefined };
			})
		);
		stats = results;
		loading = false;
	});
</script>

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
				<div class="stat-value">{stats.length}</div>
			</div>
			<div class="stat-card">
				<div class="stat-label">Total Cards</div>
				<div class="stat-value">{totalCards.toLocaleString()}</div>
			</div>
			{#if app.pricingEnabled && totalValue > 0}
				<div class="stat-card">
					<div class="stat-label">Total Value</div>
					<div class="stat-value">${totalValue.toFixed(2)}</div>
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
			<div style="padding: 8px 20px 4px;">
				<h2 style="font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text2);">Your Collections</h2>
			</div>
			<div class="collections-overview">
				{#each stats as col}
					<a href="/collection/{encodeURIComponent(col.id)}" class="collection-card">
						<div class="collection-card-name">{col.id}</div>
						<div class="collection-card-stats">
							<div class="collection-card-stat">
								<div class="collection-card-stat-val">{col.count.toLocaleString()}</div>
								<div class="collection-card-stat-lbl">Cards</div>
							</div>
							{#if app.pricingEnabled && col.value != null && col.value > 0}
								<div class="collection-card-stat">
									<div class="collection-card-stat-val">${col.value.toFixed(2)}</div>
									<div class="collection-card-stat-lbl">Value</div>
								</div>
							{/if}
						</div>
					</a>
				{/each}
			</div>
		{/if}

		<div style="padding: 24px 20px 0; display: flex; gap: 12px;">
			<a href="/search" class="btn btn-accent">
				<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
					<circle cx="6" cy="6" r="4.5" stroke="currentColor" stroke-width="1.5"/>
					<path d="M10 10l2.5 2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
				</svg>
				Search Cards
			</a>
		</div>
	{/if}
</div>
