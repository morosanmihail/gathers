<script lang="ts">
	// The art view's side panel: the hovered card's image and common details,
	// with the caller's own rows (prices and owned copies for a collection,
	// have/want for a deck) in `children`.
	import type { Snippet } from 'svelte';
	import type { AnyCard, CollectionCard, DeckCard } from '$lib/types';
	import { cardImageUrl, rarityClass } from '$lib/types';
	import { cachedImageUrl, syncCachedImageUrl } from '$lib/imageCache';
	import { app } from '$lib/state.svelte';

	interface Props {
		card: AnyCard | CollectionCard | DeckCard;
		onclick?: () => void;
		children?: Snippet;
	}

	let { card, onclick, children }: Props = $props();

	const setName = $derived(
		card.setCode
			? app.cardSets.find(s => s.code.toLowerCase() === card.setCode!.toLowerCase())?.name ?? ''
			: ''
	);

	const rawImgUrl = $derived(cardImageUrl(card as Parameters<typeof cardImageUrl>[0]));
	// Not every card shape has an artist (a deck row doesn't).
	const artist = $derived((card as { artist?: string }).artist);
	let imgUrl = $state('');
	$effect(() => {
		const raw = rawImgUrl;
		if (!raw) { imgUrl = ''; return; }
		imgUrl = syncCachedImageUrl(raw) || '';
		cachedImageUrl(raw).then(u => { if (rawImgUrl === raw && imgUrl !== u) imgUrl = u; });
	});
</script>

<aside class="art-panel" aria-label="Card details">
	<button class="art-panel-img" onclick={() => onclick?.()} title="Open details">
		{#if imgUrl}
			<img src={imgUrl} alt={card.name} />
		{:else}
			<div class="art-panel-placeholder">🃏</div>
		{/if}
	</button>

	<h3 class="art-panel-name">{card.name}</h3>

	<dl class="art-panel-rows">
		{#if card.setCode}
			<dt>Set</dt>
			<dd>{setName ? `${setName} ` : ''}({card.setCode.toUpperCase()}){card.collectorNumber ? ` #${card.collectorNumber}` : ''}</dd>
		{/if}
		{#if card.rarity}
			<dt>Rarity</dt>
			<dd><span class={rarityClass(card.rarity)}>{card.rarity}</span></dd>
		{/if}
		{#if artist}
			<dt>Artist</dt>
			<dd>{artist}</dd>
		{/if}
		{@render children?.()}
	</dl>

	<p class="art-panel-hint">Click a card to edit quantities and see price history.</p>
</aside>

<style>
	.art-panel {
		position: sticky;
		top: 94px;
		max-height: calc(100vh - 94px);
		overflow-y: auto;
		padding: 20px;
		border-left: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.art-panel-img {
		display: block;
		width: 100%;
		padding: 0;
		border: none;
		background: none;
		cursor: pointer;
	}

	.art-panel-img img {
		width: 100%;
		display: block;
		border-radius: 4.5% / 3.2%;
		box-shadow: var(--shadow);
	}

	.art-panel-placeholder {
		aspect-ratio: 5 / 7;
		border-radius: var(--radius-lg);
		background: var(--surface2);
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 2.5rem;
	}

	.art-panel-name {
		margin: 0;
		font-size: 1.05rem;
	}

	.art-panel-rows {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 6px 12px;
		margin: 0;
		font-size: 0.85rem;
	}

	/* The caller's rows come in through a snippet, so they're styled here
	   rather than in a scope they don't belong to. */
	.art-panel-rows :global(dt) {
		color: var(--text2);
		font-size: 0.7rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		padding-top: 2px;
	}

	.art-panel-rows :global(dd) {
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
	}

	.art-panel-rows :global(.muted) { color: var(--text2); }

	.art-panel-hint {
		margin: 0;
		color: var(--text3);
		font-size: 0.75rem;
	}

	/* No room for a side panel (and no hover on touch): just the art. */
	@media (max-width: 900px) {
		.art-panel { display: none; }
	}
</style>
