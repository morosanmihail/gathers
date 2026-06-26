<script lang="ts">
	import { portal } from '$lib/portal';
	import type { AnyCard, CollectionCard } from '$lib/types';
	import { cardImageUrl } from '$lib/types';
	import { cachedImageUrl, syncCachedImageUrl } from '$lib/imageCache';

	interface Props {
		card: AnyCard | CollectionCard;
	}

	let { card }: Props = $props();

	let visible = $state(false);
	let tooltipStyle = $state('');
	let hideTimer: ReturnType<typeof setTimeout>;

	const rawUrl = $derived(cardImageUrl(card as Parameters<typeof cardImageUrl>[0]));
	let resolvedUrl = $state('');

	$effect(() => {
		resolvedUrl = syncCachedImageUrl(rawUrl);
		if (rawUrl && !resolvedUrl) {
			cachedImageUrl(rawUrl).then(u => { resolvedUrl = u; });
		}
	});

	function showEl(el: HTMLElement) {
		clearTimeout(hideTimer);
		if (!rawUrl) return;
		visible = true;
		position(el);
	}

	function show(e: MouseEvent) { showEl(e.currentTarget as HTMLElement); }

	function hide() {
		hideTimer = setTimeout(() => { visible = false; }, 80);
	}

	function position(el: HTMLElement) {
		const rect = el.getBoundingClientRect();
		const cardW = 200;
		const cardH = 280;
		const margin = 12;
		let top = rect.top + rect.height / 2 - cardH / 2;
		top = Math.max(margin, Math.min(top, window.innerHeight - cardH - margin));
		const spaceRight = window.innerWidth - rect.right;
		if (spaceRight >= cardW + margin) {
			tooltipStyle = `top:${top}px; left:${rect.right + margin}px;`;
		} else {
			tooltipStyle = `top:${top}px; left:${rect.left - cardW - margin}px;`;
		}
	}
</script>

<span
	class="card-img-trigger"
	role="button"
	tabindex="0"
	onmouseenter={show}
	onmouseleave={hide}
	onkeydown={(e) => { if (e.key === 'Enter') showEl(e.currentTarget as HTMLElement); if (e.key === 'Escape') hide(); }}
>
	{card.name}
</span>

{#if visible && resolvedUrl}
	<div use:portal class="card-img-tooltip" style={tooltipStyle}>
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
