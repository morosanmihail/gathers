<script lang="ts">
	import type { CollectionCard, MtgCard, AnyCard, CardPrices } from '$lib/types';
	import { app } from '$lib/state.svelte';
	import PriceTooltip from './PriceTooltip.svelte';

	interface Props {
		card: AnyCard | CollectionCard;
		collectionMode?: boolean;
		collection?: string;
		price?: string | null;
		cardPrices?: CardPrices;
		onAdd?: (card: AnyCard | CollectionCard) => void;
		onAdjust?: (card: CollectionCard, delta: number, foil: boolean) => void;
		onclick?: (card: AnyCard | CollectionCard) => void;
	}

	let { card, collectionMode = false, collection = '', price = null, cardPrices, onAdd, onAdjust, onclick }: Props = $props();

	const col = $derived(card as CollectionCard);
	const isSelected = $derived(app.selectedCards.has(card.id));

	function rarityClass(r?: string) {
		if (!r) return 'rarity';
		return `rarity rarity-${r[0].toUpperCase()}`;
	}
</script>

<div
	class="card-row"
	class:selected={isSelected}
	role="row"
	tabindex="0"
	onclick={() => onclick?.(card)}
	onkeydown={(e) => e.key === 'Enter' && onclick?.(card)}
>
	<!-- Select checkbox -->
	<div class="card-row-cell" style="display:flex;align-items:center;justify-content:center;">
		{#if collectionMode}
			<div
				class="card-tile-select"
				class:checked={isSelected}
				style="position:relative;opacity:1;"
				role="checkbox"
				aria-checked={isSelected}
				tabindex="0"
				onclick={(e) => { e.stopPropagation(); app.toggleSelected(card.id); }}
				onkeydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); app.toggleSelected(card.id); } }}
			>
				{#if isSelected}
					<svg width="10" height="10" viewBox="0 0 10 10" fill="white">
						<path d="M1.5 5L4 7.5 8.5 2" stroke="white" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
					</svg>
				{/if}
			</div>
		{/if}
	</div>

	<div class="card-row-cell card-row-name">{card.name}</div>
	<div class="card-row-cell card-row-set mono">{card.setCode ?? '—'}</div>
	<div class="card-row-cell card-row-rarity">
		{#if card.rarity}<span class={rarityClass(card.rarity)}>{card.rarity}</span>{:else}—{/if}
	</div>
	<div class="card-row-cell card-row-artist">{(card as MtgCard).artist ?? '—'}</div>

	{#if collectionMode}
		<div class="card-row-cell" style="padding:0 8px;">
			<PriceTooltip cardId={card.id} {collection} {price} {cardPrices} />
		</div>
		<!-- Qty column: normal quantity -->
		<div class="card-row-cell" style="padding: 2px 4px;" onclick={(e) => e.stopPropagation()}>
			{#if onAdjust}
				<div class="qty-row">
					<button class="qty-btn" disabled={col.quantity <= 0} onclick={() => onAdjust(col, -1, false)}>−</button>
					<span class="qty-val">{col.quantity ?? 0}</span>
					<button class="qty-btn add" onclick={() => onAdjust(col, 1, false)}>+</button>
				</div>
			{:else}
				<span class="card-row-qty">{col.quantity ?? 0}</span>
			{/if}
		</div>
		<!-- Foil column: foil quantity -->
		<div class="card-row-cell" style="padding: 2px 4px;" onclick={(e) => e.stopPropagation()}>
			{#if onAdjust}
				<div class="qty-row">
					<button class="qty-btn" disabled={col.foilQuantity <= 0} onclick={() => onAdjust(col, -1, true)}>−</button>
					<span class="qty-val qty-foil">{col.foilQuantity ?? 0}✦</span>
					<button class="qty-btn add" onclick={() => onAdjust(col, 1, true)}>+</button>
				</div>
			{:else}
				<span class="card-row-qty qty-foil">{col.foilQuantity ?? 0}✦</span>
			{/if}
		</div>
	{:else}
		<div class="card-row-cell card-row-artist">{(card as MtgCard).text?.slice(0, 60) ?? ''}</div>
		<div class="card-row-cell"></div>
		<div class="card-row-cell">
			{#if onAdd}
				<button
					class="btn btn-sm btn-accent"
					onclick={(e) => { e.stopPropagation(); onAdd(card); }}
				>+</button>
			{/if}
		</div>
	{/if}
</div>
