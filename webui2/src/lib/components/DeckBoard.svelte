<script lang="ts">
	// One board of a deck: its cards plus the controls for changing quantities and shuffling
	// cards between boards. Honours the global grid/list toggle, like every other card view.
	//
	// Deliberately not CardResultsList: a deck row has no price column but does have a board and
	// an availability note, so reusing the collection row would have meant adding a deck flag to it
	// and to everything it renders.
	import type { CardAvailability, DeckCard, TransferDirection } from '$lib/types';
	import { deckSlots, finishLabel } from '$lib/types';
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
		/** Every board this deck has, for the "move to" control. */
		boards: string[];
		/** Plain collections, for the "take from" control. */
		collections?: string[];
		onAdjust: (card: DeckCard, delta: number, field?: 'quantity' | 'wantQuantity') => void;
		onMove: (card: DeckCard, toBoard: string) => void;
		onTransfer?: (card: DeckCard, collection: string, direction: TransferDirection) => void;
		onclick?: (card: DeckCard) => void;
	}

	let {
		deck, board, cards, availability = {}, boards, collections = [],
		onAdjust, onMove, onTransfer, onclick
	}: Props = $props();

	// Wanting four and holding three is four cards of the board, not seven.
	const count = $derived(cards.reduce((n, c) => n + deckSlots(c), 0));
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

	/** Which collections hold a copy, so a wanted card says where to go and find one. */
	function inCollections(card: DeckCard) {
		return availability[card.id]?.copies ?? [];
	}
</script>

<section class="deck-board">
	<header class="deck-board-header">
		<h2 class="deck-board-title">{board}</h2>
		<span class="deck-board-count">{count} card{count === 1 ? '' : 's'}</span>
	</header>

	{#if cards.length === 0}
		<p class="deck-board-empty">Nothing on this board yet.</p>
	{:else if app.viewMode === 'grid'}
		<div class="card-grid">
			{#each cards as card (card.id + ':' + (card.finish ?? '') + ':' + (card.board ?? ''))}
				<DeckTile
					{card}
					{otherBoards}
					others={elsewhere(card)}
					short={shortfall(card)}
					owned={inCollections(card)}
					{onAdjust}
					{onMove}
					{onclick}
				/>
			{/each}
		</div>
	{:else}
		<div class="deck-row-list">
			{#each cards as card (card.id + ':' + (card.finish ?? '') + ':' + (card.board ?? ''))}
				{@const others = elsewhere(card)}
				{@const short = shortfall(card)}
				{@const owned = inCollections(card)}
				{@const missing = Math.max(0, (card.wantQuantity ?? 0) - (card.quantity ?? 0))}
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
						{#if missing > 0}
							<span class="deck-badge deck-badge-want" title="Slots this board still has to fill">
								{missing} to find
							</span>
						{/if}
						<!-- The review's case: say not just that copies are missing, but where to find them -->
						{#each owned as c (c.collection)}
							<span class="deck-badge" title="Copies you own in this collection">
								{c.quantity} in {c.collection}
							</span>
						{/each}
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
									<option value={b}>{b}</option>
								{/each}
							</select>
						{/if}

						{#if onTransfer && collections.length > 0}
							<!-- A transfer, unlike the steppers above: the copy leaves the collection.
							     Each option carries its direction by name, so the two halves of the
							     list can't be told apart by a sign. -->
							<select
								class="deck-move-select"
								title="Move a physical copy in from a collection, or give one back"
								value=""
								onchange={(e) => {
									const choice = e.currentTarget.value;
									e.currentTarget.value = '';
									if (!choice) return;
									const separator = choice.indexOf(':');
									onTransfer(
										card,
										choice.slice(separator + 1),
										choice.slice(0, separator) as TransferDirection
									);
								}}
							>
								<option value="">Collection…</option>
								{#each collections as c (c)}
									<option value={'intoDeck:' + c}>Take 1 from {c}</option>
								{/each}
								{#each collections as c (c)}
									<option value={'outOfDeck:' + c} disabled={(card.quantity ?? 0) <= 0}>
										Give 1 to {c}
									</option>
								{/each}
							</select>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</section>
