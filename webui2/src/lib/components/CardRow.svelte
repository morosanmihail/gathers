<script lang="ts">
	import type { CollectionCard, MtgCard, AnyCard, CardPrices } from '$lib/types';
	import { app } from '$lib/state.svelte';
	import PriceTooltip from './PriceTooltip.svelte';
	import QtyControls from './QtyControls.svelte';
	import SetTooltip from './SetTooltip.svelte';
	import CardImageTooltip from './CardImageTooltip.svelte';

	interface Props {
		card: AnyCard | CollectionCard;
		collectionMode?: boolean;
		collection?: string;
		price?: string | null;
		cardPrices?: CardPrices;
		onAdd?: (card: AnyCard | CollectionCard) => void;
		onAddFoil?: (card: AnyCard | CollectionCard) => void;
		onAdjust?: (card: CollectionCard, delta: number, foil: boolean, purchasePrice?: number | null) => void;
		onclick?: (card: AnyCard | CollectionCard) => void;
	}

	let { card, collectionMode = false, collection = '', price = null, cardPrices, onAdd, onAddFoil, onAdjust, onclick }: Props = $props();

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
				style="position:relative;top:0;left:0;opacity:1;"
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

	{#if collectionMode}
		<div class="card-row-cell card-row-name"><CardImageTooltip {card} /></div>
		<div class="card-row-cell card-row-set mono"><SetTooltip setCode={card.setCode} /></div>
	{:else}
		<div class="card-row-cell card-row-set mono"><SetTooltip setCode={card.setCode} /></div>
		<div class="card-row-cell card-row-name"><CardImageTooltip {card} /></div>
	{/if}
	<div class="card-row-cell card-row-rarity">
		{#if card.rarity}<span class={rarityClass(card.rarity)}>{collectionMode ? card.rarity : card.rarity[0].toUpperCase()}</span>{:else}—{/if}
	</div>
	{#if collectionMode}
	<div class="card-row-cell card-row-artist">{(card as MtgCard).artist ?? '—'}</div>
	{/if}

	{#if collectionMode}
		<div class="card-row-cell" style="padding:0 8px;">
			<PriceTooltip cardId={card.id} {collection} {price} {cardPrices} />
		</div>
		<!-- Qty + foil columns span both cells when editing price -->
		<div class="card-row-cell" style="padding: 2px 4px; grid-column: span 2;" onclick={(e) => e.stopPropagation()}>
			{#if onAdjust}
				<QtyControls
					quantity={col.quantity ?? 0}
					foilQuantity={col.foilQuantity ?? 0}
					{price}
					onAdjust={(delta, foil, purchasePrice) => onAdjust(col, delta, foil, purchasePrice)}
				/>
			{:else}
				<div style="display:flex;gap:8px;">
					<span class="card-row-qty">{col.quantity ?? 0}</span>
					<span class="card-row-qty qty-foil">{col.foilQuantity ?? 0}✦</span>
				</div>
			{/if}
		</div>
	{:else}
		<div class="card-row-cell" style="padding:0 8px;">
			<PriceTooltip cardId={card.id} {collection} {price} {cardPrices} />
		</div>
		<div class="card-row-cell" style="display:flex;gap:6px;" onclick={(e) => e.stopPropagation()}>
			{#if onAdd}
				<button
					class="btn btn-sm btn-accent"
					title="Add to collection"
					onclick={() => onAdd(card)}
				>+</button>
			{/if}
			{#if onAddFoil}
				<button
					class="btn btn-sm btn-ghost"
					title="Add as foil"
					onclick={() => onAddFoil(card)}
				>+✦</button>
			{/if}
		</div>
	{/if}
</div>
