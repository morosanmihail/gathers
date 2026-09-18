<script lang="ts">
	// +/- stepper for a single card finish (e.g. the "" / normal row, or a
	// "foil" row) — one of these renders per finish in `FinishList`, rather
	// than a single component owning a hardcoded normal+foil pair.
	interface Props {
		quantity: number;
		onAdjust: (delta: number, purchasePrice?: number | null) => void;
		price?: string | null;
		label?: string;
		busy?: boolean;
	}

	let { quantity, onAdjust, price = null, label, busy = false }: Props = $props();

	// Pending add: waiting for user to confirm purchase price
	let pending = $state(false);
	let priceStr = $state('');

	function startAdd() {
		// Strip currency symbol if present
		priceStr = price?.replace(/[^0-9.]/g, '') ?? '';
		pending = true;
	}

	function confirmAdd() {
		const parsed = priceStr !== '' ? parseFloat(priceStr) : null;
		const purchasePrice = parsed != null && isFinite(parsed) && parsed > 0 ? parsed : null;
		onAdjust(1, purchasePrice);
		pending = false;
	}

	function cancelAdd() { pending = false; }

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') { e.preventDefault(); confirmAdd(); }
		if (e.key === 'Escape') cancelAdd();
	}
</script>

<div class="qty-controls" role="presentation" onclick={(e) => e.stopPropagation()}>
	{#if pending}
		<!-- Price confirmation row -->
		<div class="qty-row" style="gap:4px; flex-wrap: nowrap;">
			{#if label}
				<span style="font-size:0.72rem; color:var(--text2); white-space:nowrap;">{label} price:</span>
			{/if}
			<div style="display:flex; align-items:center; gap:3px;">
				<span style="color:var(--text2); font-size:0.82rem;">$</span>
				<!-- svelte-ignore a11y_autofocus -->
				<input
					class="input"
					type="number"
					min="0"
					step="0.01"
					placeholder="0.00"
					bind:value={priceStr}
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
			{#if label}<span class="qty-finish-label">{label}</span>{/if}
			<button class="qty-btn" disabled={busy || quantity <= 0} onclick={() => onAdjust(-1)}>−</button>
			<span class="qty-val">{quantity}</span>
			<button class="qty-btn add" disabled={busy} onclick={startAdd}>+</button>
		</div>
	{/if}
</div>
