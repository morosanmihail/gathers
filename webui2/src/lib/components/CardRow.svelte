<script lang="ts">
	import type { CollectionCard, MtgCard, AnyCard, CardPrices } from '$lib/types';
	import { rarityClass } from '$lib/types';
	import { app } from '$lib/state.svelte';
	import PriceTooltip from './PriceTooltip.svelte';
	import QtyControls from './QtyControls.svelte';
	import SetTooltip from './SetTooltip.svelte';
	import CardImageTooltip from './CardImageTooltip.svelte';
	import SelectCheckbox from './SelectCheckbox.svelte';

	interface Props {
		card: AnyCard | CollectionCard;
		collectionMode?: boolean;
		selectable?: boolean;
		collection?: string;
		price?: string | null;
		cardPrices?: CardPrices;
		onAdd?: (card: AnyCard | CollectionCard) => void;
		onAddFoil?: (card: AnyCard | CollectionCard) => void;
		onAdjust?: (card: CollectionCard, delta: number, foil: boolean, purchasePrice?: number | null) => void;
		onclick?: (card: AnyCard | CollectionCard) => void;
	}

	let { card, collectionMode = false, selectable = true, collection = '', price = null, cardPrices, onAdd, onAddFoil, onAdjust, onclick }: Props = $props();

	const col = $derived(card as CollectionCard);
	const isSelected = $derived(app.selectedCards.has(card.id));
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
		{#if collectionMode && selectable}
			<SelectCheckbox
				checked={isSelected}
				ontoggle={() => app.toggleSelected(card.id)}
				style="position:relative;top:0;left:0;opacity:1;"
			/>
		{/if}
	</div>

	<div class="card-row-cell card-row-name"><CardImageTooltip {card} /></div>
	<div class="card-row-cell card-row-set mono"><SetTooltip setCode={card.setCode} /></div>
	<div class="card-row-cell card-row-rarity">
		{#if card.rarity}<span class={rarityClass(card.rarity)}>{collectionMode ? card.rarity : card.rarity[0].toUpperCase()}</span>{:else}—{/if}
	</div>
	{#if collectionMode}
	<div class="card-row-cell card-row-artist">{(card as MtgCard).artist ?? '—'}</div>
	{/if}

	<div class="card-row-cell" style="padding:0 8px;">
		<PriceTooltip cardId={card.id} {collection} {price} {cardPrices} />
	</div>
	{#if collectionMode}
		<!-- Qty + foil columns span both cells when editing price -->
		<div class="card-row-cell" role="presentation" style="padding: 2px 4px; grid-column: span 2;" onclick={(e) => e.stopPropagation()}>
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
		<div class="card-row-cell" style="display:flex;gap:6px;">
			{#if onAdd}
				<button
					class="btn btn-sm btn-accent"
					title="Add to collection"
					onclick={(e) => { e.stopPropagation(); onAdd(card); }}
				>+</button>
			{/if}
			{#if onAddFoil}
				<button
					class="btn btn-sm btn-ghost"
					title="Add as foil"
					onclick={(e) => { e.stopPropagation(); onAddFoil(card); }}
				>+✦</button>
			{/if}
		</div>
	{/if}
</div>
