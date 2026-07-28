<script lang="ts">
	import { page } from '$app/stores';
	import CardResultsList from '$lib/components/CardResultsList.svelte';
	import CardDetailModal from '$lib/components/CardDetailModal.svelte';
	import { getPublicCollectionCards } from '$lib/api';
	import { app, setViewMode } from '$lib/state.svelte';
	import type { CollectionCard } from '$lib/types';

	// Fields the /api/share endpoint can sort by server-side (mirrors the
	// collection-entry columns, not card-level fields like name/rarity).
	const SORTABLE_FIELDS = new Set(['Quantity', 'FoilQuantity']);

	const token = $derived(decodeURIComponent($page.params.token ?? ''));

	let cards = $state<CollectionCard[]>([]);
	let total = $state(0);
	let currentPage = $state(1);
	let loading = $state(true);
	let error = $state('');
	let sortBy = $state('');
	let sortOrder = $state<'Asc' | 'Desc'>('Asc');
	let detailCard = $state<CollectionCard | null>(null);

	const collectionId = $derived(cards[0]?.collectionId ?? '');

	async function load(p = currentPage) {
		loading = true;
		error = '';
		try {
			const result = await getPublicCollectionCards(token, p, sortBy, sortOrder);
			cards = result.cards;
			total = result.total;
		} catch (e) {
			error = String(e);
			cards = [];
			total = 0;
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		token;
		currentPage = 1;
		load(1);
	});

	function handlePageChange(p: number) {
		currentPage = p;
		load(p);
		window.scrollTo({ top: 0, behavior: 'smooth' });
	}

	function handleSortClick(field: string) {
		if (!SORTABLE_FIELDS.has(field)) return;
		if (sortBy === field) {
			sortOrder = sortOrder === 'Asc' ? 'Desc' : 'Asc';
		} else {
			sortBy = field;
			sortOrder = 'Asc';
		}
		currentPage = 1;
		load(1);
	}

	const listHeaders = [
		{ field: '',             label: '' },
		{ field: 'Name',         label: 'Name' },
		{ field: 'SetCode',      label: 'Set' },
		{ field: 'Rarity',       label: 'Rarity' },
		{ field: 'Quantity',     label: 'Qty' },
		{ field: 'FoilQuantity', label: 'Foil' },
	];
</script>

<svelte:head>
	<title>{collectionId || 'Shared collection'} - gatheRs (shared)</title>
</svelte:head>

<div class="app-shell">
	<header class="main-header">
		<div class="header-top">
			<span class="header-logo brand">
				GatheRs
				<span>Shared collection</span>
			</span>
			<div class="header-spacer"></div>
			<div class="view-toggle" title="Toggle view">
				<button
					class="view-toggle-btn"
					class:active={app.viewMode === 'grid'}
					onclick={() => setViewMode('grid')}
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
					class:active={app.viewMode === 'list'}
					onclick={() => setViewMode('list')}
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
		</div>
	</header>

	<main class="content">
		<div class="page-header">
			<h1 class="page-title">{collectionId || 'Shared collection'}</h1>
			{#if !loading && !error}
				<span class="page-subtitle">{total.toLocaleString()} card{total !== 1 ? 's' : ''}</span>
			{/if}
		</div>

		{#if error}
			<div class="empty-state">
				<div class="empty-state-icon">🔒</div>
				<div class="empty-state-text">Couldn't load this collection: {error}</div>
			</div>
		{:else if loading && cards.length === 0}
			<div class="loading-row"><div class="spinner"></div> Loading…</div>
		{:else if !loading && cards.length === 0}
			<div class="empty-state">
				<div class="empty-state-icon">📦</div>
				<div class="empty-state-text">No cards in this collection.</div>
			</div>
		{:else}
			<CardResultsList
				{cards}
				viewMode={app.viewMode}
				{listHeaders}
				keyFn={(c) => c.id}
				collectionMode
				selectable={false}
				onclick={(c) => detailCard = c as CollectionCard}
				{sortBy}
				{sortOrder}
				onSortClick={handleSortClick}
				{total}
				page={currentPage}
				onPageChange={handlePageChange}
			/>

			{#if loading}
				<div class="loading-row"><div class="spinner"></div></div>
			{/if}
		{/if}
	</main>
</div>

{#if detailCard}
	<CardDetailModal card={detailCard} onclose={() => detailCard = null} />
{/if}
