<script lang="ts">
	import { portal } from '$lib/portal';
	import type { AnyCard, CollectionCard } from '$lib/types';
	import { cardImageUrl } from '$lib/types';
	import { cachedImageUrl, syncCachedImageUrl } from '$lib/imageCache';
	import { createHoverTooltip } from '$lib/tooltip.svelte';

	interface Props {
		card: AnyCard | CollectionCard;
	}

	let { card }: Props = $props();

	const tooltip = createHoverTooltip(80);

	const rawUrl = $derived(cardImageUrl(card as Parameters<typeof cardImageUrl>[0]));
	let resolvedUrl = $state('');

	$effect(() => {
		resolvedUrl = syncCachedImageUrl(rawUrl);
		if (rawUrl && !resolvedUrl) {
			cachedImageUrl(rawUrl).then(u => { resolvedUrl = u; });
		}
	});

	function position(el: HTMLElement) {
		const rect = el.getBoundingClientRect();
		const cardW = 200;
		const cardH = 280;
		const margin = 12;
		let top = rect.top + rect.height / 2 - cardH / 2;
		top = Math.max(margin, Math.min(top, window.innerHeight - cardH - margin));
		const spaceRight = window.innerWidth - rect.right;
		return spaceRight >= cardW + margin
			? `top:${top}px; left:${rect.right + margin}px;`
			: `top:${top}px; left:${rect.left - cardW - margin}px;`;
	}

	function showEl(el: HTMLElement) {
		if (!rawUrl) return;
		tooltip.showEl(el, position);
	}

	function show(e: MouseEvent) { showEl(e.currentTarget as HTMLElement); }
</script>

<span
	class="card-img-trigger"
	role="button"
	tabindex="0"
	onmouseenter={show}
	onmouseleave={tooltip.hide}
	onkeydown={(e) => { if (e.key === 'Enter') showEl(e.currentTarget as HTMLElement); if (e.key === 'Escape') tooltip.hide(); }}
>
	{card.name}
</span>

{#if tooltip.visible && resolvedUrl}
	<div use:portal class="card-img-tooltip" style={tooltip.style}>
		<img src={resolvedUrl} alt={card.name} width="200" />
	</div>
{/if}

<style>
	.card-img-trigger {
		cursor: default;
	}

	.card-img-tooltip {
		position: fixed;
		z-index: 9999;
		border-radius: 10px;
		overflow: hidden;
		box-shadow: 0 8px 32px rgba(0,0,0,0.5);
		pointer-events: none;
		line-height: 0;
	}

	.card-img-tooltip img {
		display: block;
		width: 200px;
		height: auto;
		border-radius: 10px;
	}
</style>
