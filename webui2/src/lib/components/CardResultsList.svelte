<script lang="ts">
	import CardTile from './CardTile.svelte';
	import CardRow from './CardRow.svelte';
	import Pagination from './Pagination.svelte';
	import type { AnyCard, CollectionCard, CardPrices, ViewMode } from '$lib/types';
	import { bestPrice } from '$lib/types';

	interface ListHeader {
		field: string;
		label: string;
	}

	interface Props {
		cards: (AnyCard | CollectionCard)[];
		viewMode: ViewMode;
		listHeaders: ListHeader[];
		keyFn: (card: AnyCard | CollectionCard) => string;
		collectionMode?: boolean;
		selectable?: boolean;
		collection?: string;
		prices?: Record<string, CardPrices>;
		onAdd?: (card: AnyCard | CollectionCard) => void;
		onAddFoil?: (card: AnyCard | CollectionCard) => void;
		onAddWanted?: (card: AnyCard | CollectionCard) => void;
		onAdjust?: (card: CollectionCard, delta: number, foil: boolean, purchasePrice?: number | null) => void;
		onWantAdjust?: (card: CollectionCard, delta: number) => void;
		onclick?: (card: AnyCard | CollectionCard) => void;
		sortBy?: string;
		sortOrder?: 'Asc' | 'Desc';
		onSortClick?: (field: string) => void;
		total: number;
		page: number;
		onPageChange: (p: number) => void;
		gridClass?: string;
		gridStyle?: string;
		listClass?: string;
	}

	let {
		cards, viewMode, listHeaders, keyFn, collectionMode = false, selectable = true, collection = '',
		prices = {}, onAdd, onAddFoil, onAddWanted, onAdjust, onWantAdjust, onclick, sortBy = '', sortOrder = 'Asc',
		onSortClick, total, page, onPageChange, gridClass = 'card-grid', gridStyle = '', listClass = 'card-list'
	}: Props = $props();
</script>

{#if viewMode === 'grid'}
	<div class={gridClass} style={gridStyle}>
		{#each cards as card (keyFn(card))}
			<CardTile {card} {collectionMode} {selectable} {collection} price={bestPrice(prices[card.id])} cardPrices={prices[card.id]} {onAdd} {onAddFoil} {onAddWanted} {onAdjust} {onWantAdjust} {onclick} />
		{/each}
	</div>
{:else}
	<div class={listClass}>
		<div class="card-list-header">
			{#each listHeaders as h}
				{#if onSortClick}
					<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
					<div
						class="card-list-col"
						class:active={h.field !== '' && sortBy === h.field}
						role={h.field ? 'button' : undefined}
						tabindex={h.field ? 0 : undefined}
						onclick={() => h.field && onSortClick(h.field)}
						onkeydown={(e) => e.key === 'Enter' && h.field && onSortClick(h.field)}
					>
						{h.label}
						{#if h.field && sortBy === h.field}
							{sortOrder === 'Asc' ? ' ↑' : ' ↓'}
						{/if}
					</div>
				{:else}
					<div class="card-list-col">{h.label}</div>
				{/if}
			{/each}
		</div>
		{#each cards as card (keyFn(card))}
			<CardRow {card} {collectionMode} {selectable} {collection} price={bestPrice(prices[card.id])} cardPrices={prices[card.id]} {onAdd} {onAddFoil} {onAddWanted} {onAdjust} {onWantAdjust} {onclick} />
		{/each}
	</div>
{/if}

<Pagination {total} {page} onchange={onPageChange} />
