<script lang="ts">
	// One card of the art view: just the image, with `copies` drawn as a pile
	// behind it. Shared by collections (`CardArtGrid`) and decks (`DeckBoard`),
	// so it only takes what to draw, not how a copy is counted.
	import type { AnyCard, CollectionCard, DeckCard } from '$lib/types';
	import { cardImageUrl } from '$lib/types';
	import { cachedImageUrl, syncCachedImageUrl } from '$lib/imageCache';
	import SelectCheckbox from './SelectCheckbox.svelte';

	// Copies drawn behind the front card; more than this just reads as "a pile".
	const MAX_BACK_LAYERS = 4;

	interface Props {
		card: AnyCard | CollectionCard | DeckCard;
		copies: number;
		/** Corner badge, e.g. "×3"; none when null. */
		badge?: string | null;
		/** Secondary badge style, for "nothing held yet" states. */
		badgeMuted?: boolean;
		/** Dimmed, for a card only wanted rather than held. */
		faded?: boolean;
		/** The card the side panel is showing. */
		active?: boolean;
		/** Shows a selection checkbox when given. */
		selected?: boolean;
		ontoggle?: () => void;
		onhover?: () => void;
		onclick?: () => void;
	}

	let {
		card, copies, badge = null, badgeMuted = false, faded = false, active = false,
		selected = false, ontoggle, onhover, onclick
	}: Props = $props();

	const backLayers = $derived(Math.min(Math.max(copies - 1, 0), MAX_BACK_LAYERS));
	// Deepest first, so the front card paints on top.
	const backDepths = $derived(Array.from({ length: backLayers }, (_, i) => backLayers - i));

	const rawImgUrl = $derived(cardImageUrl(card as Parameters<typeof cardImageUrl>[0]));
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
	class="art-tile"
	class:selected
	class:active
	class:want-only={faded}
	role="button"
	tabindex="0"
	aria-label="{card.name}{copies > 1 ? `, ${copies} copies` : ''}"
	style="--back-layers: {backLayers};"
	onmouseenter={() => onhover?.()}
	onfocus={() => onhover?.()}
	onclick={() => onclick?.()}
	onkeydown={(e) => e.key === 'Enter' && onclick?.()}
>
	<div class="art-stack">
		<!-- Back copy at depth k sits k steps up and to the right of the front card. -->
		{#each backDepths as k (k)}
			<div class="art-layer art-back" style="--k: {k};" aria-hidden="true">
				{#if imgUrl}<img src={imgUrl} alt="" />{:else}<div class="art-placeholder"></div>{/if}
			</div>
		{/each}
		<div class="art-layer art-front">
			{#if imgUrl}
				<img src={imgUrl} alt={card.name} />
			{:else}
				<div class="art-placeholder">
					<div>
						<div style="font-size: 2em; margin-bottom: 0.25em;">🃏</div>
						<div style="font-weight:700;">{card.name ?? '…'}</div>
					</div>
				</div>
			{/if}
			{#if ontoggle}
				<SelectCheckbox checked={selected} {ontoggle} />
			{/if}
			{#if badge}
				<div class="art-count" class:art-count-want={badgeMuted}>{badge}</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.art-tile {
		/* Pile offset, badge and placeholder text all follow the zoom. */
		--step: calc(4px * var(--art-scale, 1));
		position: relative;
		cursor: pointer;
		outline: none;
		/* Room for the largest stack, so every tile's front card lines up. */
		padding: calc(var(--step) * 4) calc(var(--step) * 4) 0 0;
	}

	.art-stack {
		position: relative;
		transition: transform 0.15s;
	}

	.art-tile:hover .art-stack,
	.art-tile:focus-visible .art-stack { transform: translateY(-3px); }

	.art-layer {
		border-radius: 4.5% / 3.2%;
		overflow: hidden;
		background: var(--surface2);
		aspect-ratio: 5 / 7;
	}

	.art-layer img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}

	.art-back {
		position: absolute;
		inset: 0;
		transform: translate(calc(var(--k) * var(--step)), calc(var(--k) * var(--step) * -1));
		box-shadow: var(--shadow-sm);
		filter: brightness(calc(1 - var(--k) * 0.12));
	}

	.art-front {
		position: relative;
		box-shadow: var(--shadow-sm);
		border: 2px solid transparent;
		transition: border-color 0.15s, box-shadow 0.15s;
	}

	.art-tile:hover .art-front,
	.art-tile.active .art-front { box-shadow: var(--shadow); }

	.art-tile:focus-visible .art-front { border-color: var(--border2); }

	.art-tile.selected .art-front {
		border-color: var(--accent);
		box-shadow: 0 0 0 2px var(--accent-glow), var(--shadow);
	}

	.art-tile.want-only { opacity: 0.5; }
	.art-tile.want-only:hover { opacity: 0.8; }

	.art-tile:hover :global(.card-tile-select),
	.art-tile.selected :global(.card-tile-select) { opacity: 1; }

	.art-placeholder {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		text-align: center;
		padding: 8px;
		color: var(--text2);
		font-size: clamp(0.62rem, calc(0.78rem * var(--art-scale, 1)), 1rem);
		overflow: hidden;
	}

	.art-count {
		position: absolute;
		bottom: clamp(4px, calc(6px * var(--art-scale, 1)), 10px);
		right: clamp(4px, calc(6px * var(--art-scale, 1)), 10px);
		background: var(--accent);
		color: #0d0d12;
		font-size: clamp(0.62rem, calc(0.75rem * var(--art-scale, 1)), 1rem);
		font-weight: 700;
		padding: 0.15em 0.55em;
		border-radius: 999px;
		box-shadow: var(--shadow-sm);
	}

	.art-count-want {
		background: var(--surface2);
		color: var(--text);
		border: 1px solid var(--border2);
	}
</style>
