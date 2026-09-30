<script lang="ts">
	// Grid form of a deck row: the card image, with the quantity stepper under it and the move
	// control revealed on hover.
	//
	// Separate from `CardTile`, for the same reason `DeckBoard` is separate from
	// `CardResultsList`: the quantity badge there is built from a `CardGroup`'s entries, which a
	// deck card has no equivalent of.
	import type { CardAvailability, DeckCard } from '$lib/types';
	import { boardLabel, cardImageUrl, finishLabel } from '$lib/types';
	import { cachedImageUrl, syncCachedImageUrl } from '$lib/imageCache';
	import SetTooltip from './SetTooltip.svelte';

	interface Props {
		card: DeckCard;
		/** Boards other than the one this tile sits on, for the move control. */
		otherBoards: string[];
		/** Copies in other built decks; see `DeckBoard.elsewhere`. */
		others?: CardAvailability['holdings'];
		/** Copies committed beyond what is owned; see `DeckBoard.shortfall`. */
		short?: number;
		onAdjust: (card: DeckCard, delta: number) => void;
		onMove: (card: DeckCard, toBoard: string) => void;
		onclick?: (card: DeckCard) => void;
	}

	let { card, otherBoards, others = [], short = 0, onAdjust, onMove, onclick }: Props = $props();

	const rawImgUrl = $derived(cardImageUrl(card as Parameters<typeof cardImageUrl>[0]));

	// Same two-step load as `CardTile`: the synchronous cache hit avoids a flash of placeholder
	// on a card already seen, and the async call fills in the rest.
	let imgUrl = $state('');
	$effect(() => {
		const raw = rawImgUrl;
		if (!raw) { imgUrl = ''; return; }
		const cached = syncCachedImageUrl(raw);
		if (cached) imgUrl = cached;
		cachedImageUrl(raw).then((u) => { if (imgUrl !== u) imgUrl = u; });
	});
</script>

<div
	class="card-tile deck-tile"
	role="button"
	tabindex="0"
	onclick={() => onclick?.(card)}
	onkeydown={(e) => e.key === 'Enter' && onclick?.(card)}
>
	<div class="card-tile-qty">
		{card.quantity ?? 0}{#if card.finish}&nbsp;{finishLabel(card.finish)}{/if}
	</div>

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

	<div class="card-tile-body">
		<div class="card-tile-name" title={card.name}>{card.name}</div>
		{#if card.setCode}
			<div class="card-tile-meta"><SetTooltip setCode={card.setCode} /></div>
		{/if}
	</div>

	{#if short > 0 || others.length > 0}
		<div class="deck-row-note deck-tile-note">
			{#if short > 0}
				<span class="deck-badge deck-badge-warn" title="More copies are committed to built decks than are owned">
					&#9888; {short} short
				</span>
			{/if}
			{#each others as h (h.deck + ':' + h.board + ':' + h.finish)}
				<span class="deck-badge" title="This many copies are in a deck that is currently built">
					{h.quantity} in {h.deck}{h.location ? ` ()` : ''}
				</span>
			{/each}
		</div>
	{/if}

	<div class="deck-tile-controls" role="presentation" onclick={(e) => e.stopPropagation()}>
		<div class="qty-row">
			<button class="qty-btn" disabled={(card.quantity ?? 0) <= 0} onclick={() => onAdjust(card, -1)}>−</button>
			<span class="qty-val">{card.quantity ?? 0}</span>
			<button class="qty-btn add" onclick={() => onAdjust(card, 1)}>+</button>
		</div>
		{#if otherBoards.length > 0}
			<select
				class="deck-move-select deck-tile-move"
				title="Move one copy to another board"
				value=""
				onchange={(e) => {
					const target = e.currentTarget.value;
					e.currentTarget.value = '';
					if (target) onMove(card, target);
				}}
			>
				<option value="">Move to…</option>
				{#each otherBoards as b (b)}
					<option value={b}>{boardLabel(b)}</option>
				{/each}
			</select>
		{/if}
	</div>
</div>
