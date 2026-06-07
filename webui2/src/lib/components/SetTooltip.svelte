<script lang="ts">
	import { portal } from '$lib/portal';
	import { app } from '$lib/state.svelte';

	interface Props {
		setCode?: string;
	}

	let { setCode }: Props = $props();

	let visible = $state(false);
	let tooltipStyle = $state('');
	let hideTimer: ReturnType<typeof setTimeout>;

	const setName = $derived(
		setCode
			? (app.cardSets.find(s => s.code.toLowerCase() === setCode.toLowerCase())?.name ?? setCode.toUpperCase())
			: '—'
	);

	function show(e: MouseEvent) {
		clearTimeout(hideTimer);
		visible = true;
		position(e.currentTarget as HTMLElement);
	}

	function hide() {
		hideTimer = setTimeout(() => { visible = false; }, 120);
	}

	function position(el: HTMLElement) {
		const rect = el.getBoundingClientRect();
		const tooltipW = 220;
		const spaceRight = window.innerWidth - rect.right;
		if (spaceRight >= tooltipW + 8) {
			tooltipStyle = `top: ${rect.bottom + 4}px; left: ${rect.left}px;`;
		} else {
			tooltipStyle = `top: ${rect.bottom + 4}px; right: ${spaceRight}px;`;
		}
	}
</script>

<span class="set-code-trigger" onmouseenter={show} onmouseleave={hide}>
	{setCode ? setCode.toUpperCase() : '—'}
</span>

{#if visible}
	<div use:portal class="price-tooltip set-tooltip" style={tooltipStyle}>
		<div class="price-tooltip-title">Set</div>
		<div class="set-tooltip-name">{setName}</div>
	</div>
{/if}

<style>
	.set-code-trigger {
		cursor: default;
	}
</style>
