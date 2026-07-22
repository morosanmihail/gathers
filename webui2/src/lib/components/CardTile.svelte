<script lang="ts">
	import type { CollectionCard, MtgCard, AnyCard, CardPrices } from '$lib/types';
	import { cardImageUrl, rarityClass } from '$lib/types';
	import { cachedImageUrl, syncCachedImageUrl } from '$lib/imageCache';
	import { app } from '$lib/state.svelte';
	import QtyControls from './QtyControls.svelte';
	import PriceTooltip from './PriceTooltip.svelte';
	import SetTooltip from './SetTooltip.svelte';
	import SelectCheckbox from './SelectCheckbox.svelte';

	interface Props {
		card: AnyCard | CollectionCard;
		collectionMode?: boolean;
		price?: string | null;
		cardPrices?: CardPrices;
		collection?: string;
		onAdd?: (card: AnyCard | CollectionCard) => void;
		onAddFoil?: (card: AnyCard | CollectionCard) => void;
		onAdjust?: (card: CollectionCard, delta: number, foil: boolean, purchasePrice?: number | null) => void;
		onclick?: (card: AnyCard | CollectionCard) => void;
	}

	let { card, collectionMode = false, price = null, cardPrices, collection = '', onAdd, onAddFoil, onAdjust, onclick }: Props = $props();

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
	role="button"
	tabindex="0"
	onclick={() => onclick?.(card)}
	onkeydown={(e) => e.key === 'Enter' && onclick?.(card)}
>
	<!-- Selection checkbox -->
	{#if collectionMode}
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

	<!-- Add to collection buttons (search mode) -->
	{#if !collectionMode && (onAdd || onAddFoil)}
		<div class="card-tile-add" style="display:flex;gap:4px;">
			{#if onAdd}
				<button
					class="btn btn-sm btn-accent"
					onclick={(e) => { e.stopPropagation(); onAdd(card); }}
					title="Add to collection"
				>+</button>
			{/if}
			{#if onAddFoil}
				<button
					class="btn btn-sm btn-ghost"
					onclick={(e) => { e.stopPropagation(); onAddFoil(card); }}
					title="Add as foil"
				>+✦</button>
			{/if}
		</div>
	{/if}
</div>
