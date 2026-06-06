<script lang="ts">
	interface Props {
		quantity: number;
		foilQuantity: number;
		onAdjust: (delta: number, foil: boolean) => void;
		busy?: boolean;
	}

	let { quantity, foilQuantity, onAdjust, busy = false }: Props = $props();
</script>

<div class="qty-controls" onclick={(e) => e.stopPropagation()}>
	<div class="qty-row">
		<button class="qty-btn" disabled={busy || quantity <= 0} onclick={() => onAdjust(-1, false)}>−</button>
		<span class="qty-val">{quantity}</span>
		<button class="qty-btn add" disabled={busy} onclick={() => onAdjust(1, false)}>+</button>
	</div>
	<div class="qty-row">
		<button class="qty-btn" disabled={busy || foilQuantity <= 0} onclick={() => onAdjust(-1, true)}>−</button>
		<span class="qty-val qty-foil">{foilQuantity}✦</span>
		<button class="qty-btn add" disabled={busy} onclick={() => onAdjust(1, true)}>+</button>
	</div>
</div>
