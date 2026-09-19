<script lang="ts">
	import type { CardGroup } from '$lib/types';
	import { finishLabel, availableFinishesToAdd } from '$lib/types';
	import QtyControls from './QtyControls.svelte';

	interface Props {
		group: CardGroup;
		price?: string | null;
		onAdjust: (finish: string, delta: number, purchasePrice?: number | null) => void;
		busy?: boolean;
	}

	let { group, price = null, onAdjust, busy = false }: Props = $props();

	// Only show finish rows the card actually owns copies of. A group can
	// also include a wishlist-only "" entry (quantity 0, want > 0) — the
	// Wanted control (rendered separately by the caller) already covers
	// that, so fall back to a bare "" row only when nothing is owned at all.
	const ownedEntries = $derived(
		group.entries
			.filter((e) => (e.quantity ?? 0) > 0)
			.map((e) => ({ finish: e.finish ?? '', quantity: e.quantity ?? 0 }))
	);
	const rows = $derived(ownedEntries.length ? ownedEntries : [{ finish: '', quantity: 0 }]);

	const toAdd = $derived(availableFinishesToAdd(group));
	let addingFinish = $state<string | null>(null);
</script>

<div style="display:flex; flex-direction:column; gap:2px;" role="presentation" onclick={(e) => e.stopPropagation()}>
	{#each rows as entry (entry.finish ?? '')}
		<QtyControls
			quantity={entry.quantity ?? 0}
			label={finishLabel(entry.finish ?? '')}
			{price}
			{busy}
			onAdjust={(delta, purchasePrice) => onAdjust(entry.finish ?? '', delta, purchasePrice)}
		/>
	{/each}

	{#if toAdd.length > 0}
		{#if addingFinish !== null}
			<div class="add-finish-row">
				<select class="add-finish-select" bind:value={addingFinish}>
					{#each toAdd as f (f)}
						<option value={f}>{finishLabel(f)}</option>
					{/each}
				</select>
				<button class="qty-btn add" title="Add this finish" onclick={() => { onAdjust(addingFinish ?? '', 1); addingFinish = null; }}>✓</button>
				<button class="qty-btn" title="Cancel" onclick={() => (addingFinish = null)}>✕</button>
			</div>
		{:else}
			<button
				class="qty-btn add"
				style="width:auto; padding:0 6px; font-size:0.68rem;"
				title="Add another finish/version of this card"
				onclick={() => (addingFinish = toAdd[0])}
			>
				+ version
			</button>
		{/if}
	{/if}
</div>
