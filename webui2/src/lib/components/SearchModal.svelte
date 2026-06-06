<script lang="ts">
	import SearchPanel from './SearchPanel.svelte';
	import CardTile from './CardTile.svelte';
	import CardRow from './CardRow.svelte';
	import Pagination from './Pagination.svelte';
	import { searchMtg, searchRiftbound, searchPokemon, addCardToCollection, PAGE_SIZE } from '$lib/api';
	import { app } from '$lib/state.svelte';
	import { defaultFilters } from '$lib/types';
	import type { AnyCard, CollectionCard } from '$lib/types';

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
	let activeSystem = $state('');

	$effect(() => {
		if (!activeSystem && app.systems.length > 0) activeSystem = app.systems[0];
	});

	function handleSystemChange(sys: string) {
		activeSystem = sys;
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
			if (p === 1) total = data.length >= PAGE_SIZE ? PAGE_SIZE * 10 : data.length;
			searched = true;
		} finally {
			loading = false;
		}
	}

	async function addCard(card: AnyCard | CollectionCard) {
		try {
			await app.withOp(`Adding ${card.name}`, () =>
				addCardToCollection(collection, card.id)
			);
			toast = `Added ${card.name}`;
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

<div class="modal-overlay" onclick={onOverlayClick} role="dialog" aria-modal="true">
	<div class="modal">
		<div class="modal-header">
			<h3>Search & Add to "{collection}"</h3>
			<button class="btn btn-ghost btn-icon" onclick={onclose} title="Close">✕</button>
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
					{#if app.viewMode === 'grid'}
						<div class="card-grid" style="padding: 0; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 10px;">
							{#each results as card (card.id)}
								<CardTile {card} onAdd={addCard} />
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
								<div class="card-list-col"></div>
								<div class="card-list-col"></div>
								<div class="card-list-col"></div>
							</div>
							{#each results as card (card.id)}
								<CardRow {card} onAdd={addCard} />
							{/each}
						</div>
					{/if}
					<Pagination {total} {page} onchange={(p) => doSearch(p)} />
				{/if}
			</div>
		</div>
	</div>
</div>
