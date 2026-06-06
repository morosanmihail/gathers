<script lang="ts">
	import { getPurchaseHistory, type PurchaseEntry } from '$lib/api';
	import type { CardPrices } from '$lib/types';
	import { portal } from '$lib/portal';

	interface Props {
		cardId: string;
		collection: string;
		price: string | null;
		cardPrices?: CardPrices;
	}

	let { cardId, collection, price, cardPrices }: Props = $props();

	let visible = $state(false);
	let entries = $state<PurchaseEntry[] | null>(null);
	let loading = $state(false);
	let tooltipStyle = $state('');
	let hideTimer: ReturnType<typeof setTimeout>;

	async function show(e: MouseEvent) {
		clearTimeout(hideTimer);
		visible = true;
		position(e.currentTarget as HTMLElement);
		if (entries === null && !loading && collection) {
			loading = true;
			entries = await getPurchaseHistory(collection, cardId);
			loading = false;
		}
	}

	function hide() {
		hideTimer = setTimeout(() => { visible = false; }, 120);
	}

	function position(el: HTMLElement) {
		const rect = el.getBoundingClientRect();
		// Prefer opening to the left; flip if too close to right edge
		const tooltipW = 260;
		const spaceRight = window.innerWidth - rect.right;
		if (spaceRight >= tooltipW + 8) {
			tooltipStyle = `top: ${rect.bottom + 4}px; left: ${rect.left}px;`;
		} else {
			tooltipStyle = `top: ${rect.bottom + 4}px; right: ${spaceRight}px;`;
		}
	}

	// Market price rows from all providers
	const providerRows = $derived.by(() => {
		if (!cardPrices?.paper) return [];
		return Object.entries(cardPrices.paper)
			.map(([retailer, rp]) => ({ retailer, normal: rp.normal, foil: rp.foil }))
			.filter(r => r.normal != null || r.foil != null)
			.sort((a, b) => {
				const aMin = Math.min(a.normal ?? Infinity, a.foil ?? Infinity);
				const bMin = Math.min(b.normal ?? Infinity, b.foil ?? Infinity);
				return aMin - bMin;
			});
	});

	function fmtDate(iso: string) {
		try { return new Date(iso).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }); }
		catch { return iso; }
	}

	function fmtQty(e: PurchaseEntry) {
		const parts: string[] = [];
		if (e.quantity > 0) parts.push(`${e.quantity}×`);
		if (e.foil_quantity > 0) parts.push(`${e.foil_quantity}✦`);
		return parts.join(' ') || '—';
	}

	function fmtHistPrice(e: PurchaseEntry) {
		const p = e.normal_price_per_unit ?? e.foil_price_per_unit;
		return p != null ? `$${p.toFixed(2)}` : '—';
	}
</script>

<div class="price-cell" onmouseenter={show} onmouseleave={hide}>
	{price ?? ''}
</div>

{#if visible}
	<div use:portal class="price-tooltip" style={tooltipStyle} onmouseenter={() => clearTimeout(hideTimer)} onmouseleave={hide}>
		<!-- Market prices -->
		{#if providerRows.length > 0}
			<div class="price-tooltip-title">Market prices</div>
			{#each providerRows as row}
				<div class="price-tooltip-row">
					<span class="price-tooltip-date" style="text-transform:capitalize;">{row.retailer}</span>
					{#if row.normal != null}
						<span class="price-tooltip-qty">normal</span>
						<span class="price-tooltip-val">${row.normal.toFixed(2)}</span>
					{:else}
						<span class="price-tooltip-qty">foil</span>
						<span class="price-tooltip-val">${row.foil!.toFixed(2)}✦</span>
					{/if}
				</div>
				{#if row.normal != null && row.foil != null}
					<div class="price-tooltip-row">
						<span class="price-tooltip-date"></span>
						<span class="price-tooltip-qty">foil</span>
						<span class="price-tooltip-val">${row.foil.toFixed(2)}✦</span>
					</div>
				{/if}
			{/each}
		{:else if price}
			<div class="price-tooltip-empty">No provider breakdown</div>
		{:else}
			<div class="price-tooltip-empty">No market price</div>
		{/if}

		<!-- Purchase history -->
		{#if collection}
			<div class="price-tooltip-title" style="margin-top: 10px;">Purchase history</div>
			{#if loading}
				<div class="price-tooltip-empty">Loading…</div>
			{:else if !entries?.length}
				<div class="price-tooltip-empty">None recorded</div>
			{:else}
				{#each entries as entry}
					<div class="price-tooltip-row">
						<span class="price-tooltip-date">{fmtDate(entry.recorded_at)}</span>
						<span class="price-tooltip-qty">{fmtQty(entry)}</span>
						<span class="price-tooltip-val">{fmtHistPrice(entry)}</span>
					</div>
				{/each}
			{/if}
		{/if}
	</div>
{/if}
