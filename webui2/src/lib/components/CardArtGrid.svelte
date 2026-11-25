<script lang="ts">
	import type { CardGroup, CardPrices } from '$lib/types';
	import { finishLabel, isWantOnly } from '$lib/types';
	import { formatFinishPrice } from '$lib/currency.svelte';
	import { app, artZoomStyle } from '$lib/state.svelte';
	import CardArtTile from './CardArtTile.svelte';
	import CardArtPanel from './CardArtPanel.svelte';

	// Collection "art" view: image-only tiles, owned copies stacked, and the
	// hovered card's details in a panel on the right. Layout classes live in
	// app.css, shared with the deck page's art view.
	interface Props {
		cards: CardGroup[];
		keyFn: (card: CardGroup) => string;
		selectable?: boolean;
		prices?: Record<string, CardPrices>;
		onclick?: (card: CardGroup) => void;
	}

	let { cards, keyFn, selectable = true, prices = {}, onclick }: Props = $props();

	// Remember the card by id, so the panel follows quantity edits (and a
	// re-sorted page) instead of showing a stale snapshot. It stays on the
	// last hovered card rather than flickering empty between tiles.
	let hoveredId = $state<string | null>(null);
	const hovered = $derived(cards.find(c => c.id === hoveredId) ?? cards[0] ?? null);

	// Every owned copy across all finishes is one card in the stack.
	const copiesOf = (c: CardGroup) => c.entries.reduce((n, e) => n + (e.quantity ?? 0), 0);

	const owned = $derived(hovered?.entries.filter(e => (e.quantity ?? 0) > 0) ?? []);
	const totalOwned = $derived(owned.reduce((n, e) => n + (e.quantity ?? 0), 0));
	const priceRows = $derived(
		hovered
			? (owned.length ? owned.map(e => e.finish ?? '') : [''])
				.map(f => ({ finish: f, text: formatFinishPrice(prices[hovered.id], f) }))
				.filter((r): r is { finish: string; text: string } => !!r.text)
			: []
	);
</script>

<div class="art-view" style={artZoomStyle()}>
	<div class="art-grid">
		{#each cards as card (keyFn(card))}
			{@const copies = copiesOf(card)}
			{@const wantOnly = isWantOnly(card)}
			<CardArtTile
				{card}
				{copies}
				badge={copies > 1 ? `×${copies}` : wantOnly ? `Want ${card.wantQuantity}` : null}
				badgeMuted={wantOnly}
				faded={wantOnly}
				active={hovered?.id === card.id}
				selected={app.selectedCards.has(card.id)}
				ontoggle={selectable ? () => app.toggleSelected(card.id) : undefined}
				onhover={() => hoveredId = card.id}
				onclick={() => onclick?.(card)}
			/>
		{/each}
	</div>

	{#if hovered}
		<CardArtPanel card={hovered} onclick={() => onclick?.(hovered)}>
			{#if priceRows.length}
				<dt>Price</dt>
				<dd>
					{#each priceRows as row (row.finish)}
						<div>{#if priceRows.length > 1}<span class="muted">{finishLabel(row.finish, hovered)}</span> {/if}<strong>{row.text}</strong></div>
					{/each}
				</dd>
			{/if}
			<dt>Owned</dt>
			<dd>
				{#if owned.length === 0}
					<span class="muted">None</span>
				{:else}
					{#each owned as e (e.finish ?? '')}
						<div>{e.quantity} × {finishLabel(e.finish ?? '', hovered)}</div>
					{/each}
					{#if owned.length > 1}<div class="muted">{totalOwned} total</div>{/if}
				{/if}
			</dd>
			{#if (hovered.wantQuantity ?? 0) > 0}
				<dt>Wanted</dt>
				<dd>{hovered.wantQuantity}</dd>
			{/if}
		</CardArtPanel>
	{/if}
</div>
