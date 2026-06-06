<script lang="ts">
	import type { CollectionCard, MtgCard, AnyCard, CardPrices } from '$lib/types';
	import { cardImageUrl } from '$lib/types';
	import { cachedImageUrl } from '$lib/imageCache';
	import { app } from '$lib/state.svelte';
	import QtyControls from './QtyControls.svelte';
	import PriceTooltip from './PriceTooltip.svelte';

	interface Props {
		card: AnyCard | CollectionCard;
		collectionMode?: boolean;
		price?: string | null;
		cardPrices?: CardPrices;
		collection?: string;
		onAdd?: (card: AnyCard | CollectionCard) => void;
		onAdjust?: (card: CollectionCard, delta: number, foil: boolean) => void;
		onclick?: (card: AnyCard | CollectionCard) => void;
	}

	let { card, collectionMode = false, price = null, cardPrices, collection = '', onAdd, onAdjust, onclick }: Props = $props();

	const col = $derived(card as CollectionCard);
	const isSelected = $derived(app.selectedCards.has(card.id));
	const rawImgUrl = $derived(cardImageUrl(card as CollectionCard));
	const qty = $derived(collectionMode && col.quantity != null
		? col.foilQuantity > 0 ? `${col.quantity} + ${col.foilQuantity}✦` : `${col.quantity}`
		: null);

	// Cached image URL — resolves async, starts as the raw URL
	let imgUrl = $state('');
	$effect(() => {
		const raw = rawImgUrl;
		if (!raw) { imgUrl = ''; return; }
		cachedImageUrl(raw).then(u => { imgUrl = u; });
	});

	function rarityClass(r?: string) {
		if (!r) return '';
		const f = r[0].toUpperCase();
		return `rarity rarity-${f}`;
	}
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
		<div
			class="card-tile-select"
			class:checked={isSelected}
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
				<span>{card.setCode.toUpperCase()}</span>
			{/if}
			{#if card.rarity}
				<span class={rarityClass(card.rarity)}>{card.rarity[0]}</span>
			{/if}
			{#if price != null || cardPrices}
				<span style="margin-left: auto;" onclick={(e) => e.stopPropagation()}>
					<PriceTooltip cardId={card.id} {collection} {price} {cardPrices} />
				</span>
			{/if}
		</div>
	</div>

	<!-- Qty controls (collection mode) -->
	{#if collectionMode && onAdjust}
		<div style="padding: 4px 8px 6px; border-top: 1px solid var(--border);">
			<QtyControls
				quantity={col.quantity ?? 0}
				foilQuantity={col.foilQuantity ?? 0}
				onAdjust={(delta, foil) => onAdjust(col, delta, foil)}
			/>
		</div>
	{/if}

	<!-- Add to collection button (search mode) -->
	{#if !collectionMode && onAdd}
		<div class="card-tile-add">
			<button
				class="btn btn-sm btn-accent"
				onclick={(e) => { e.stopPropagation(); onAdd(card); }}
				title="Add to collection"
			>+</button>
		</div>
	{/if}
</div>
