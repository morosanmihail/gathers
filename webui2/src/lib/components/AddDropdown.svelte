<script lang="ts">
	import { portal } from '$lib/portal';
	import { clampHorizontal } from '$lib/tooltip.svelte';

	interface Props {
		onAdd?: () => void;
		onAddFoil?: () => void;
		onAddWanted?: () => void;
	}

	let { onAdd, onAddFoil, onAddWanted }: Props = $props();

	let open = $state(false);
	let style = $state('');
	let btnEl: HTMLButtonElement | undefined = $state();

	function position() {
		if (!btnEl) return;
		const rect = btnEl.getBoundingClientRect();
		const menuH = 108;
		const margin = 6;
		const xStyle = clampHorizontal(rect, 140);
		const fitsBelow = rect.bottom + margin + menuH <= window.innerHeight;
		const yStyle = fitsBelow
			? `top: ${rect.bottom + margin}px;`
			: `bottom: ${window.innerHeight - rect.top + margin}px;`;
		style = `${yStyle} ${xStyle}`;
	}

	function toggle(e: MouseEvent) {
		e.stopPropagation();
		open = !open;
		if (open) position();
	}

	function pick(e: MouseEvent, fn?: () => void) {
		e.stopPropagation();
		open = false;
		fn?.();
	}
</script>

<button bind:this={btnEl} class="btn btn-sm btn-accent" title="Add to collection" onclick={toggle}>
	+ ▾
</button>

{#if open}
	<div
		use:portal
		class="add-dropdown-menu"
		role="menu"
		tabindex="-1"
		{style}
		onclick={(e) => e.stopPropagation()}
		onkeydown={(e) => { if (e.key === 'Escape') open = false; }}
	>
		{#if onAdd}
			<button class="add-dropdown-item" role="menuitem" onclick={(e) => pick(e, onAdd)}>Add 1</button>
		{/if}
		{#if onAddFoil}
			<button class="add-dropdown-item" role="menuitem" onclick={(e) => pick(e, onAddFoil)}>Add 1 foil</button>
		{/if}
		{#if onAddWanted}
			<button class="add-dropdown-item" role="menuitem" onclick={(e) => pick(e, onAddWanted)}>Add 1 wanted</button>
		{/if}
	</div>
{/if}

<svelte:window onclick={() => { if (open) open = false; }} />
