<script lang="ts">
	// One board of a deck: its cards plus the controls for changing quantities and shuffling
	// cards between boards. Honours the global grid/list toggle, like every other card view.
	//
	// Deliberately not `CardResultsList`: a deck row has no price column and no want quantity, but
	// does have a board and an availability note, so reusing the collection row would have meant
	// adding a deck flag to it and to everything it renders.
	import type { CardAvailability, DeckCard } from '$lib/types';
	import { boardLabel, finishLabel } from '$lib/types';
	import { app } from '$lib/state.svelte';
	import CardImageTooltip from './CardImageTooltip.svelte';
	import DeckTile from './DeckTile.svelte';
	import SetTooltip from './SetTooltip.svelte';

	interface Props {
		deck: string;
		board: string;
		cards: DeckCard[];
		/** Keyed by card id; absent while the lookup is still in flight. */
		availability?: Record<string, CardAvailability>;
		/** Every board this deck uses, for the "move to" control. */
		boards: string[];
		onAdjust: (card: DeckCard, delta: number) => void;
		onMove: (card: DeckCard, toBoard: string) => void;
		onclick?: (card: DeckCard) => void;
	}

	let { deck, board, cards, availability = {}, boards, onAdjust, onMove, onclick }: Props = $props();

	const count = $derived(cards.reduce((n, c) => n + (c.quantity ?? 0), 0));
	const otherBoards = $derived(boards.filter((b) => b !== board));

	/**
	 * Copies of this card sitting in *other* built decks.
	 *
	 * The deck being viewed is excluded: naming it is noise, and the other decks
	 * are the ones that would have to be raided.
	 */
	function elsewhere(card: DeckCard) {
		return (availability[card.id]?.holdings ?? []).filter((h) => h.deck !== deck);
	}

	function shortfall(card: DeckCard): number {
		const a = availability[card.id];
		if (!a) return 0;
		return Math.max(0, a.committed - a.owned);
	}
</script>

<section class="deck-board">
	<header class="deck-board-header">
		<h2 class="deck-board-title">{boardLabel(board)}</h2>
		<span class="deck-board-count">{count} card{count === 1 ? '' : 's'}</span>
	</header>

	{#if cards.length === 0}
		<p class="deck-board-empty">Nothing on this board yet.</p>
	{:else if app.viewMode === 'grid'}
		<div class="card-grid">
			{#each cards as card (card.id + ':' + (card.finish ?? '') + ':' + (card.board ?? ''))}
				<DeckTile {card} {otherBoards} others={elsewhere(card)} short={shortfall(card)} {onAdjust} {onMove} {onclick} />
			{/each}
		</div>
	{:else}
		<div class="deck-row-list">
			{#each cards as card (card.id + ':' + (card.finish ?? '') + ':' + (card.board ?? ''))}
				{@const others = elsewhere(card)}
				{@const short = shortfall(card)}
				<div
					class="deck-row"
					role="row"
					tabindex="0"
					onclick={() => onclick?.(card)}
					onkeydown={(e) => e.key === 'Enter' && onclick?.(card)}
				>
					<div class="deck-row-name">
						<CardImageTooltip {card} />
						{#if card.finish}
							<span class="deck-row-finish">{finishLabel(card.finish)}</span>
						{/if}
					</div>

					<div class="deck-row-set mono"><SetTooltip setCode={card.setCode} /></div>

					<div class="deck-row-note">
						{#if short > 0}
							<span class="deck-badge deck-badge-warn" title="More copies are committed to built decks than you own">
								⚠ {short} short
							</span>
						{/if}
						{#each others as h (h.deck + ':' + h.board + ':' + h.finish)}
							<span class="deck-badge" title="This many copies are in a deck that's currently built">
								{h.quantity} in {h.deck}{h.location ? ` (${h.location})` : ''}
							</span>
						{/each}
					</div>

					<div class="deck-row-controls" role="presentation" onclick={(e) => e.stopPropagation()}>
						<div class="qty-row">
							<button class="qty-btn" disabled={(card.quantity ?? 0) <= 0} onclick={() => onAdjust(card, -1)}>−</button>
							<span class="qty-val">{card.quantity ?? 0}</span>
							<button class="qty-btn add" onclick={() => onAdjust(card, 1)}>+</button>
						</div>
						{#if otherBoards.length > 0}
							<select
								class="deck-move-select"
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
			{/each}
		</div>
	{/if}
</section>
