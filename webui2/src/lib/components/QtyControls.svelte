<script lang="ts">
	interface Props {
		quantity: number;
		foilQuantity: number;
		onAdjust: (delta: number, foil: boolean, purchasePrice?: number | null) => void;
		price?: string | null;
		busy?: boolean;
	}

	let { quantity, foilQuantity, onAdjust, price = null, busy = false }: Props = $props();

	// Pending add: waiting for user to confirm purchase price
	let pending = $state<{ foil: boolean; priceStr: string } | null>(null);

	function startAdd(foil: boolean) {
		// Strip currency symbol if present
		const raw = price?.replace(/[^0-9.]/g, '') ?? '';
		pending = { foil, priceStr: raw };
	}

	function confirmAdd() {
		if (!pending) return;
		const parsed = pending.priceStr !== '' ? parseFloat(pending.priceStr) : null;
		const purchasePrice = parsed != null && isFinite(parsed) && parsed > 0 ? parsed : null;
		onAdjust(1, pending.foil, purchasePrice);
		pending = null;
	}

	function cancelAdd() { pending = null; }

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') { e.preventDefault(); confirmAdd(); }
		if (e.key === 'Escape') cancelAdd();
	}
</script>

<div class="qty-controls" onclick={(e) => e.stopPropagation()}>
	{#if pending}
		<!-- Price confirmation row -->
		<div class="qty-row" style="gap:4px; flex-wrap: nowrap;">
			<span style="font-size:0.72rem; color:var(--text2); white-space:nowrap;">
				{pending.foil ? 'Foil' : 'Normal'} price:
			</span>
			<div style="display:flex; align-items:center; gap:3px;">
				<span style="color:var(--text2); font-size:0.82rem;">$</span>
				<input
					class="input"
					type="number"
					min="0"
					step="0.01"
					placeholder="0.00"
					bind:value={pending.priceStr}
					onkeydown={onKeydown}
					style="width:72px; height:24px; padding:2px 6px; font-size:0.82rem; font-family:'JetBrains Mono',monospace;"
					autofocus
				/>
				<button class="qty-btn add" onclick={confirmAdd} title="Confirm">✓</button>
				<button class="qty-btn" onclick={cancelAdd} title="Cancel">✕</button>
			</div>
		</div>
	{:else}
		<div class="qty-row">
			<button class="qty-btn" disabled={busy || quantity <= 0} onclick={() => onAdjust(-1, false)}>−</button>
			<span class="qty-val">{quantity}</span>
			<button class="qty-btn add" disabled={busy} onclick={() => startAdd(false)}>+</button>
		</div>
		<div class="qty-row">
			<button class="qty-btn" disabled={busy || foilQuantity <= 0} onclick={() => onAdjust(-1, true)}>−</button>
			<span class="qty-val qty-foil">{foilQuantity}✦</span>
			<button class="qty-btn add" disabled={busy} onclick={() => startAdd(true)}>+</button>
		</div>
	{/if}
</div>
