<script lang="ts">
	// Grid form of a deck row: the card image, with the quantity stepper under it and the move
	// control revealed on hover.
	//
	// Separate from `CardTile`, for the same reason `DeckBoard` is separate from
	// `CardResultsList`: the quantity badge there is built from a `CardGroup`'s entries, which a
	// deck card has no equivalent of.
	import type { CardAvailability, DeckCard } from '$lib/types';
	import { cardImageUrl, deckSlots, finishLabel } from '$lib/types';
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
		/** Which collections hold a copy; see `DeckBoard.inCollections`. */
		owned?: CardAvailability['copies'];
		onAdjust: (card: DeckCard, delta: number, field?: 'quantity' | 'wantQuantity') => void;
		onMove: (card: DeckCard, toBoard: string) => void;
		onclick?: (card: DeckCard) => void;
	}

	let { card, otherBoards, others = [], short = 0, owned = [], onAdjust, onMove, onclick }: Props =
		$props();

	// What the board still has to fill, which is what a want is for
	const missing = $derived(Math.max(0, (card.wantQuantity ?? 0) - (card.quantity ?? 0)));

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
		{deckSlots(card)}{#if card.finish}&nbsp;{finishLabel(card.finish)}{/if}
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

	{#if short > 0 || others.length > 0 || missing > 0 || owned.length > 0}
		<div class="deck-row-note deck-tile-note">
			{#if missing > 0}
				<span class="deck-badge deck-badge-want" title="Slots this board still has to fill">
					{missing} to find
				</span>
			{/if}
			<!-- Where to go and find the missing copies, not just that they are missing -->
			{#each owned as c (c.collection)}
				<span class="deck-badge" title="Copies you own in this collection">
					{c.quantity} in {c.collection}
				</span>
			{/each}
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
		<div class="qty-group">
			<span class="qty-label" title="Copies this deck has to hand">Have</span>
			<div class="qty-row">
				<button class="qty-btn" disabled={(card.quantity ?? 0) <= 0} onclick={() => onAdjust(card, -1)}>−</button>
				<span class="qty-val">{card.quantity ?? 0}</span>
				<button class="qty-btn add" onclick={() => onAdjust(card, 1)}>+</button>
			</div>
		</div>
		<div class="qty-group">
			<span class="qty-label" title="Copies this deck calls for">Want</span>
			<div class="qty-row">
				<button
					class="qty-btn"
					disabled={(card.wantQuantity ?? 0) <= 0}
					onclick={() => onAdjust(card, -1, 'wantQuantity')}
				>−</button>
				<span class="qty-val">{card.wantQuantity ?? 0}</span>
				<button class="qty-btn add" onclick={() => onAdjust(card, 1, 'wantQuantity')}>+</button>
			</div>
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
					<option value={b}>{b}</option>
				{/each}
			</select>
		{/if}
	</div>
</div>
