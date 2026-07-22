<script lang="ts">
	import { portal } from '$lib/portal';
	import { app } from '$lib/state.svelte';
	import { createHoverTooltip, clampHorizontal } from '$lib/tooltip.svelte';

	interface Props {
		setCode?: string;
	}

	let { setCode }: Props = $props();

	const tooltip = createHoverTooltip();

	const setName = $derived(
		setCode
			? (app.cardSets.find(s => s.code.toLowerCase() === setCode.toLowerCase())?.name ?? setCode.toUpperCase())
			: '—'
	);

	function position(el: HTMLElement) {
		const rect = el.getBoundingClientRect();
		const tooltipH = 60;
		const margin = 8;
		const xStyle = clampHorizontal(rect, 220);
		const fitsBelow = rect.bottom + 4 + tooltipH + margin <= window.innerHeight;
		const top = fitsBelow ? rect.bottom + 4 : Math.max(margin, rect.top - tooltipH - 4);
		return `top: ${top}px; ${xStyle}`;
	}

	function showEl(el: HTMLElement) { tooltip.showEl(el, position); }
	function show(e: MouseEvent) { showEl(e.currentTarget as HTMLElement); }
</script>

<span class="set-code-trigger" role="button" tabindex="0" onmouseenter={show} onmouseleave={tooltip.hide} onkeydown={(e) => { if (e.key === 'Enter') showEl(e.currentTarget as HTMLElement); if (e.key === 'Escape') tooltip.hide(); }}>
	{setCode ? setCode.toUpperCase() : '—'}
</span>

{#if tooltip.visible}
	<div use:portal class="price-tooltip set-tooltip" style={tooltip.style}>
		<div class="price-tooltip-title">Set</div>
		<div class="set-tooltip-name">{setName}</div>
	</div>
{/if}

<style>
	.set-code-trigger {
		cursor: default;
	}
</style>
