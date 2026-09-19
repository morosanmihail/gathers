<script lang="ts">
	import { portal } from '$lib/portal';
	import { clampHorizontal } from '$lib/tooltip.svelte';
	import { finishLabel } from '$lib/types';

	interface Props {
		// Single generic "Add 1" — used when `finishes` is empty (no catalog
		// finish data for this game, e.g. Riftbound/Pokemon today).
		onAdd?: () => void;
		// Catalog finishes for this specific card (already mapped to gathers'
		// internal values via `catalogFinishes` — "" for the default/nonfoil
		// finish, "foil", "etched", ...). When non-empty, one "Add 1 <finish>"
		// item is shown per finish instead of the single `onAdd` item.
		finishes?: string[];
		onAddFinish?: (finish: string) => void;
		onAddWanted?: () => void;
	}

	let { onAdd, finishes = [], onAddFinish, onAddWanted }: Props = $props();

	const perFinish = $derived(finishes.length > 0 && !!onAddFinish);
	const itemCount = $derived((perFinish ? finishes.length : (onAdd ? 1 : 0)) + (onAddWanted ? 1 : 0));

	let open = $state(false);
	let style = $state('');
	let btnEl: HTMLButtonElement | undefined = $state();

	function position() {
		if (!btnEl) return;
		const itemH = 28;
		const menuH = itemCount * itemH + 8;
		const margin = 6;
		const rect = btnEl.getBoundingClientRect();
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
		{#if perFinish}
			{#each finishes as f (f)}
				<button class="add-dropdown-item" role="menuitem" onclick={(e) => pick(e, () => onAddFinish?.(f))}>
					Add 1 {finishLabel(f)}
				</button>
			{/each}
		{:else if onAdd}
			<button class="add-dropdown-item" role="menuitem" onclick={(e) => pick(e, onAdd)}>Add 1</button>
		{/if}
		{#if onAddWanted}
			<button class="add-dropdown-item" role="menuitem" onclick={(e) => pick(e, onAddWanted)}>Add 1 wanted</button>
		{/if}
	</div>
{/if}

<svelte:window onclick={() => { if (open) open = false; }} />
