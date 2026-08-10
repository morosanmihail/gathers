<script lang="ts">
	import type { CollectionCard, MtgCard, AnyCard, CardPrices } from '$lib/types';
	import { rarityClass, isWantOnly } from '$lib/types';
	import { app } from '$lib/state.svelte';
	import PriceTooltip from './PriceTooltip.svelte';
	import QtyControls from './QtyControls.svelte';
	import SetTooltip from './SetTooltip.svelte';
	import CardImageTooltip from './CardImageTooltip.svelte';
	import SelectCheckbox from './SelectCheckbox.svelte';
	import AddDropdown from './AddDropdown.svelte';

	interface Props {
		card: AnyCard | CollectionCard;
		collectionMode?: boolean;
		selectable?: boolean;
		collection?: string;
		price?: string | null;
		cardPrices?: CardPrices;
		onAdd?: (card: AnyCard | CollectionCard) => void;
		onAddFoil?: (card: AnyCard | CollectionCard) => void;
		onAddWanted?: (card: AnyCard | CollectionCard) => void;
		onAdjust?: (card: CollectionCard, delta: number, foil: boolean, purchasePrice?: number | null) => void;
		onWantAdjust?: (card: CollectionCard, delta: number) => void;
		onclick?: (card: AnyCard | CollectionCard) => void;
	}

	let { card, collectionMode = false, selectable = true, collection = '', price = null, cardPrices, onAdd, onAddFoil, onAddWanted, onAdjust, onWantAdjust, onclick }: Props = $props();

	const col = $derived(card as CollectionCard);
	const isSelected = $derived(app.selectedCards.has(card.id));
</script>

<div
	class="card-row"
	class:selected={isSelected}
	class:want-only={collectionMode && isWantOnly(col)}
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
		<div class="card-row-cell" role="presentation" style="padding: 2px 4px;" onclick={(e) => e.stopPropagation()}>
			{#if onWantAdjust}
				<div class="qty-row">
					<button class="qty-btn" disabled={(col.wantQuantity ?? 0) <= 0} onclick={() => onWantAdjust(col, -1)}>−</button>
					<span class="qty-val">{col.wantQuantity ?? 0}</span>
					<button class="qty-btn add" onclick={() => onWantAdjust(col, 1)}>+</button>
				</div>
			{:else}
				<span class="card-row-qty">{col.wantQuantity ?? 0}</span>
			{/if}
		</div>
	{:else}
		<div class="card-row-cell" role="presentation" style="display:flex;gap:6px;" onclick={(e) => e.stopPropagation()}>
			{#if onAdd || onAddFoil || onAddWanted}
				<AddDropdown
					onAdd={onAdd ? () => onAdd(card) : undefined}
					onAddFoil={onAddFoil ? () => onAddFoil(card) : undefined}
					onAddWanted={onAddWanted ? () => onAddWanted(card) : undefined}
				/>
			{/if}
		</div>
	{/if}
</div>
