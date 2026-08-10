<script lang="ts">
	import type { CollectionCard, MtgCard, AnyCard, CardPrices } from '$lib/types';
	import { cardImageUrl, rarityClass, isWantOnly } from '$lib/types';
	import { cachedImageUrl, syncCachedImageUrl } from '$lib/imageCache';
	import { app } from '$lib/state.svelte';
	import QtyControls from './QtyControls.svelte';
	import PriceTooltip from './PriceTooltip.svelte';
	import SetTooltip from './SetTooltip.svelte';
	import SelectCheckbox from './SelectCheckbox.svelte';
	import AddDropdown from './AddDropdown.svelte';

	interface Props {
		card: AnyCard | CollectionCard;
		collectionMode?: boolean;
		selectable?: boolean;
		price?: string | null;
		cardPrices?: CardPrices;
		collection?: string;
		onAdd?: (card: AnyCard | CollectionCard) => void;
		onAddFoil?: (card: AnyCard | CollectionCard) => void;
		onAddWanted?: (card: AnyCard | CollectionCard) => void;
		onAdjust?: (card: CollectionCard, delta: number, foil: boolean, purchasePrice?: number | null) => void;
		onWantAdjust?: (card: CollectionCard, delta: number) => void;
		onclick?: (card: AnyCard | CollectionCard) => void;
	}

	let { card, collectionMode = false, selectable = true, price = null, cardPrices, collection = '', onAdd, onAddFoil, onAddWanted, onAdjust, onWantAdjust, onclick }: Props = $props();

	const col = $derived(card as CollectionCard);
	const isSelected = $derived(app.selectedCards.has(card.id));
	const rawImgUrl = $derived(cardImageUrl(card as CollectionCard));
	const qty = $derived(collectionMode && col.quantity != null
		? col.foilQuantity > 0 ? `${col.quantity} + ${col.foilQuantity}✦` : `${col.quantity}`
		: null);

	let imgUrl = $state('');
	$effect(() => {
		const raw = rawImgUrl;
		if (!raw) { imgUrl = ''; return; }
		const cached = syncCachedImageUrl(raw);
		if (cached) imgUrl = cached;
		cachedImageUrl(raw).then(u => { if (imgUrl !== u) imgUrl = u; });
	});
</script>

<div
	class="card-tile"
	class:selected={isSelected}
	class:want-only={collectionMode && isWantOnly(col)}
	role="button"
	tabindex="0"
	onclick={() => onclick?.(card)}
	onkeydown={(e) => e.key === 'Enter' && onclick?.(card)}
>
	<!-- Selection checkbox -->
	{#if collectionMode && selectable}
		<SelectCheckbox checked={isSelected} ontoggle={() => app.toggleSelected(card.id)} />
	{/if}

	<!-- Qty badge -->
	{#if qty}
		<div class="card-tile-qty">{qty}</div>
	{/if}

	<!-- Image -->
	{#if imgUrl}
		<img class="card-tile-img" src={imgUrl} alt={card.name} />
	{:else}
		<div class="card-tile-img-placeholder">
			<div>
				<div style="font-size: 1.5rem; margin-bottom: 4px;">🃏</div>
				<div style="font-weight:700;">{card.name ?? '…'}</div>
			</div>
		</div>
	{/if}

	<!-- Info -->
	<div class="card-tile-body">
		<div class="card-tile-name" title={card.name}>{card.name}</div>
		<div class="card-tile-meta">
			{#if card.setCode}
				<SetTooltip setCode={card.setCode} />
			{/if}
			{#if card.rarity}
				<span class={rarityClass(card.rarity)}>{card.rarity[0]}</span>
			{/if}
		</div>
		{#if price != null || cardPrices}
			<div class="card-tile-meta" role="presentation" onclick={(e) => e.stopPropagation()}>
				<PriceTooltip cardId={card.id} {collection} {price} {cardPrices} />
			</div>
		{/if}
	</div>

	<!-- Qty controls (collection mode) -->
	{#if collectionMode && onAdjust}
		<div style="padding: 4px 8px 6px; border-top: 1px solid var(--border);">
			<QtyControls
				quantity={col.quantity ?? 0}
				foilQuantity={col.foilQuantity ?? 0}
				{price}
				onAdjust={(delta, foil, purchasePrice) => onAdjust(col, delta, foil, purchasePrice)}
			/>
		</div>
	{/if}

	{#if collectionMode && onWantAdjust}
		<div style="padding: 0 8px 6px; display:flex; align-items:center; gap:6px;">
			<span style="font-size:0.72rem; color:var(--text2);">Wanted</span>
			<button class="qty-btn" disabled={(col.wantQuantity ?? 0) <= 0} onclick={() => onWantAdjust(col, -1)}>−</button>
			<span class="qty-val">{col.wantQuantity ?? 0}</span>
			<button class="qty-btn add" onclick={() => onWantAdjust(col, 1)}>+</button>
		</div>
	{/if}

	<!-- Add to collection dropdown (search mode) -->
	{#if !collectionMode && (onAdd || onAddFoil || onAddWanted)}
		<div class="card-tile-add" role="presentation" onclick={(e) => e.stopPropagation()}>
			<AddDropdown
				onAdd={onAdd ? () => onAdd(card) : undefined}
				onAddFoil={onAddFoil ? () => onAddFoil(card) : undefined}
				onAddWanted={onAddWanted ? () => onAddWanted(card) : undefined}
			/>
		</div>
	{/if}
</div>
