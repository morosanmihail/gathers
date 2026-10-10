<script lang="ts">
	// Line chart of a card's recorded daily prices (see the server's
	// `price_history_enabled`). Renders nothing at all unless pricing and price
	// history are on and the card actually has history.
	//
	// One chart shows one finish in one currency — never two y-axes — with a
	// line per retailer. Colours come from the theme's `--chart-N` tokens,
	// assigned per retailer across the whole currency so switching finish
	// never repaints a line. Those palettes are validated for at most three
	// series, so a currency listed by more retailers shows the first three
	// (the card's preferred retailer first).
	import { getPriceHistory, type PriceHistoryEntry } from '$lib/api';
	import { app } from '$lib/state.svelte';
	import type { CardPrices } from '$lib/types';
	import { finishLabel } from '$lib/types';
	import { formatMoney, preferredCurrency, preferredRetailerName } from '$lib/currency.svelte';
	import { retailerLabel } from '$lib/format';

	interface Props {
		provider: string;
		cardId: string;
		/** Finish to show first, when the card has history for it. */
		initialFinish?: string;
		/** Current prices, to put the card's preferred retailer first. */
		currentPrices?: CardPrices | null;
	}

	let { provider, cardId, initialFinish = '', currentPrices = null }: Props = $props();

	const MAX_SERIES = 3;
	const HEIGHT = 220;
	const M = { top: 14, bottom: 26, left: 56 };
	const LABEL_GAP = 14;

	let entries = $state<PriceHistoryEntry[]>([]);
	const enabled = $derived(app.pricingEnabled && !!app.systemInfo?.price_history_enabled);

	$effect(() => {
		const key = `${provider}:${cardId}`;
		entries = [];
		if (!enabled || !provider || !cardId) return;
		getPriceHistory(provider, cardId).then(e => {
			if (`${provider}:${cardId}` === key) entries = e;
		});
	});

	// ── Selection: finish, then currency ────────────────────────────────────
	const finishes = $derived([...new Set(entries.map(e => e.finish))].sort((a, b) => (a === '' ? -1 : b === '' ? 1 : a.localeCompare(b))));
	let chosenFinish = $state<string | null>(null);
	const finish = $derived(
		chosenFinish !== null && finishes.includes(chosenFinish) ? chosenFinish
		: finishes.includes(initialFinish) ? initialFinish
		: finishes[0] ?? ''
	);

	const currencies = $derived.by(() => {
		const counts = new Map<string, number>();
		for (const e of entries) if (e.finish === finish) counts.set(e.currency, (counts.get(e.currency) ?? 0) + 1);
		const preferred = preferredCurrency();
		return [...counts].sort(([a, ca], [b, cb]) => Number(b === preferred) - Number(a === preferred) || cb - ca || a.localeCompare(b)).map(([c]) => c);
	});
	let chosenCurrency = $state<string | null>(null);
	const currency = $derived(chosenCurrency !== null && currencies.includes(chosenCurrency) ? chosenCurrency : currencies[0] ?? '');

	// ── Series ──────────────────────────────────────────────────────────────
	// Every retailer listing in this currency (any finish), preferred first —
	// a retailer's position here is its colour slot.
	const retailerOrder = $derived.by(() => {
		const preferred = currentPrices ? preferredRetailerName(currentPrices) : undefined;
		return [...new Set(entries.filter(e => e.currency === currency).map(e => e.retailer))]
			.sort((a, b) => Number(b === preferred) - Number(a === preferred) || retailerLabel(a).localeCompare(retailerLabel(b)));
	});

	interface Point { day: string; t: number; price: number }
	interface Series { retailer: string; slot: number; points: Point[] }

	const allSeries = $derived.by((): Series[] => {
		const byRetailer = new Map<string, Point[]>();
		for (const e of entries) {
			if (e.finish !== finish || e.currency !== currency) continue;
			const pts = byRetailer.get(e.retailer) ?? [];
			pts.push({ day: e.recorded_on, t: dayTime(e.recorded_on), price: e.price });
			byRetailer.set(e.retailer, pts);
		}
		return retailerOrder
			.map((retailer, slot) => ({ retailer, slot, points: (byRetailer.get(retailer) ?? []).sort((a, b) => a.t - b.t) }))
			.filter(s => s.slot < MAX_SERIES && s.points.length > 0);
	});
	const overflowRetailers = $derived(retailerOrder.slice(MAX_SERIES).filter(r => entries.some(e => e.retailer === r && e.finish === finish && e.currency === currency)));

	let hidden = $state<Set<string>>(new Set());
	const series = $derived(allSeries.filter(s => !hidden.has(s.retailer)));

	function toggleSeries(retailer: string) {
		const next = new Set(hidden);
		if (next.has(retailer)) next.delete(retailer);
		// Never hide the last visible line.
		else if (allSeries.length - next.size > 1) next.add(retailer);
		hidden = next;
	}

	const days = $derived([...new Set(series.flatMap(s => s.points.map(p => p.day)))].sort());

	// ── Scales ──────────────────────────────────────────────────────────────
	let width = $state(0);

	function dayTime(day: string): number {
		return Date.parse(`${day.slice(0, 10)}T00:00:00Z`);
	}

	function niceStep(raw: number): number {
		const pow = 10 ** Math.floor(Math.log10(raw));
		const n = raw / pow;
		return (n <= 1 ? 1 : n <= 2 ? 2 : n <= 2.5 ? 2.5 : n <= 5 ? 5 : 10) * pow;
	}

	const yDomain = $derived.by(() => {
		const prices = series.flatMap(s => s.points.map(p => p.price));
		let lo = Math.min(...prices), hi = Math.max(...prices);
		if (lo === hi) { const pad = Math.abs(lo) * 0.1 || 1; lo -= pad; hi += pad; }
		const step = niceStep((hi - lo) / 4);
		lo = Math.max(0, Math.floor(lo / step) * step);
		hi = Math.ceil(hi / step) * step;
		const ticks: number[] = [];
		for (let v = lo; v <= hi + step / 2; v += step) ticks.push(+v.toFixed(10));
		return { lo, hi, ticks };
	});

	const tFirst = $derived(days.length ? dayTime(days[0]) : 0);
	const tLast = $derived(days.length ? dayTime(days[days.length - 1]) : 0);

	// Direct end labels ride the line ends when they don't collide; otherwise
	// the legend (which carries the same latest values) does the job.
	const endLabelWidth = $derived(
		Math.max(0, ...series.map(s => fmt(s.points[s.points.length - 1].price).length)) * 7 + LABEL_GAP + 4
	);
	const plotRight = $derived(width - endLabelWidth);
	const plotW = $derived(Math.max(10, plotRight - M.left));
	const plotH = HEIGHT - M.top - M.bottom;

	const x = (t: number) => (tLast === tFirst ? M.left + plotW / 2 : M.left + ((t - tFirst) / (tLast - tFirst)) * plotW);
	const y = (v: number) => M.top + plotH - ((v - yDomain.lo) / (yDomain.hi - yDomain.lo)) * plotH;

	const spanDays = $derived((tLast - tFirst) / 86_400_000);
	const xTicks = $derived.by(() => {
		if (!days.length) return [];
		const max = Math.max(2, Math.floor(plotW / 90));
		if (days.length <= max) return days;
		const picked = new Set<string>();
		for (let i = 0; i < max; i++) picked.add(days[Math.round((i * (days.length - 1)) / (max - 1))]);
		return [...picked];
	});

	function fmtDay(day: string, long = false): string {
		const opts: Intl.DateTimeFormatOptions = long
			? { year: 'numeric', month: 'short', day: 'numeric', timeZone: 'UTC' }
			: spanDays > 300
				? { month: 'short', year: '2-digit', timeZone: 'UTC' }
				: { month: 'short', day: 'numeric', timeZone: 'UTC' };
		return new Date(dayTime(day)).toLocaleDateString(undefined, opts);
	}

	function fmt(v: number): string {
		return formatMoney(v, currency);
	}

	function linePath(s: Series): string {
		return s.points.map((p, i) => `${i ? 'L' : 'M'}${x(p.t).toFixed(1)},${y(p.price).toFixed(1)}`).join('');
	}

	function areaPath(s: Series): string {
		const base = (M.top + plotH).toFixed(1);
		const first = s.points[0], last = s.points[s.points.length - 1];
		return `${linePath(s)}L${x(last.t).toFixed(1)},${base}L${x(first.t).toFixed(1)},${base}Z`;
	}

	const endLabels = $derived.by(() => {
		const labels = series.map(s => {
			const last = s.points[s.points.length - 1];
			return { retailer: s.retailer, y: y(last.price), text: fmt(last.price) };
		});
		const sorted = [...labels].sort((a, b) => a.y - b.y);
		const collides = sorted.some((l, i) => i > 0 && l.y - sorted[i - 1].y < 13);
		return collides ? [] : labels;
	});

	// ── Summary ─────────────────────────────────────────────────────────────
	const lead = $derived(series[0]);
	const change = $derived.by(() => {
		if (!lead || lead.points.length < 2) return null;
		const first = lead.points[0].price, last = lead.points[lead.points.length - 1].price;
		return first > 0 ? ((last - first) / first) * 100 : null;
	});

	// ── Hover / keyboard ────────────────────────────────────────────────────
	let hoverIdx = $state<number | null>(null);
	const hoverDay = $derived(hoverIdx !== null ? days[hoverIdx] : null);
	const hoverRows = $derived(
		hoverDay
			? series.flatMap(s => {
				const p = s.points.find(pt => pt.day === hoverDay);
				return p ? [{ s, p }] : [];
			})
			: []
	);

	function nearestIdx(px: number): number {
		let best = 0;
		for (let i = 1; i < days.length; i++) {
			if (Math.abs(x(dayTime(days[i])) - px) < Math.abs(x(dayTime(days[best])) - px)) best = i;
		}
		return best;
	}

	function onPointer(e: PointerEvent) {
		const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
		hoverIdx = nearestIdx(e.clientX - rect.left);
	}

	function onKey(e: KeyboardEvent) {
		if (!days.length) return;
		if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
			e.preventDefault();
			const cur = hoverIdx ?? days.length - 1;
			hoverIdx = Math.min(days.length - 1, Math.max(0, cur + (e.key === 'ArrowLeft' ? -1 : 1)));
		} else if (e.key === 'Home' || e.key === 'End') {
			e.preventDefault();
			hoverIdx = e.key === 'Home' ? 0 : days.length - 1;
		}
	}

	// What a screen reader announces for the focused date.
	const valueText = $derived.by(() => {
		const day = hoverDay ?? days[days.length - 1];
		if (!day) return '';
		const values = series.flatMap(s => {
			const p = s.points.find(pt => pt.day === day);
			return p ? [`${retailerLabel(s.retailer)} ${fmt(p.price)}`] : [];
		});
		return `${fmtDay(day, true)}: ${values.join(', ')}`;
	});

	const tooltipLeft = $derived(hoverDay ? x(dayTime(hoverDay)) : 0);
	const tooltipFlip = $derived(tooltipLeft > width - 180);

	let showTable = $state(false);
</script>

{#snippet marker(slot: number, cx: number, cy: number, r = 4)}
	{#if slot === 1}
		<rect x={cx - r} y={cy - r} width={r * 2} height={r * 2} rx="1" class="mark s{slot}" />
	{:else if slot === 2}
		<path d="M{cx},{cy - r - 1}L{cx + r + 1},{cy}L{cx},{cy + r + 1}L{cx - r - 1},{cy}Z" class="mark s{slot}" />
	{:else}
		<circle {cx} {cy} {r} class="mark s{slot}" />
	{/if}
{/snippet}

{#snippet key(slot: number)}
	<svg class="key" width="22" height="12" aria-hidden="true">
		<line x1="1" y1="6" x2="21" y2="6" class="line s{slot}" />
		{@render marker(slot, 11, 6, 3.5)}
	</svg>
{/snippet}

{#if enabled && series.length > 0}
	<section class="price-history" aria-label="Price history">
		<div class="ph-header">
			<div>
				<h4>Price history</h4>
				<div class="ph-sub">
					{[
						...(allSeries.length === 1 ? [retailerLabel(allSeries[0].retailer)] : []),
						finishLabel(finish),
						currency,
						`${days.length} snapshot${days.length === 1 ? '' : 's'} since ${fmtDay(days[0], true)}`,
					].join(' · ')}
					{#if change !== null}
						· {#if series.length > 1}{retailerLabel(lead.retailer)}{' '}{/if}<span class="ph-change">{change >= 0 ? '▲' : '▼'} {Math.abs(change).toFixed(1)}%</span>
					{/if}
				</div>
			</div>
			<div class="ph-controls">
				{#if finishes.length > 1}
					<div class="seg" role="group" aria-label="Finish">
						{#each finishes as f (f)}
							<button class:active={f === finish} aria-pressed={f === finish} onclick={() => { chosenFinish = f; hoverIdx = null; }}>{finishLabel(f)}</button>
						{/each}
					</div>
				{/if}
				{#if currencies.length > 1}
					<div class="seg" role="group" aria-label="Currency">
						{#each currencies as c (c)}
							<button class:active={c === currency} aria-pressed={c === currency} onclick={() => { chosenCurrency = c; hidden = new Set(); hoverIdx = null; }}>{c}</button>
						{/each}
					</div>
				{/if}
				<button class="btn btn-ghost btn-sm" onclick={() => (showTable = !showTable)} aria-pressed={showTable}>
					{showTable ? 'Chart' : 'Table'}
				</button>
			</div>
		</div>

		{#if allSeries.length > 1}
			<div class="ph-legend">
				{#each allSeries as s (s.retailer)}
					{@const last = s.points[s.points.length - 1]}
					<button
						class="legend-item"
						class:off={hidden.has(s.retailer)}
						aria-pressed={!hidden.has(s.retailer)}
						title={hidden.has(s.retailer) ? 'Show' : 'Hide'}
						onclick={() => toggleSeries(s.retailer)}
					>
						{@render key(s.slot)}
						<span class="legend-name">{retailerLabel(s.retailer)}</span>
						<span class="legend-value">{fmt(last.price)}</span>
					</button>
				{/each}
			</div>
		{/if}

		{#if showTable}
			<div class="ph-table-wrap">
				<table class="ph-table">
					<thead>
						<tr>
							<th scope="col">Date</th>
							{#each series as s (s.retailer)}<th scope="col">{retailerLabel(s.retailer)}</th>{/each}
						</tr>
					</thead>
					<tbody>
						{#each [...days].reverse() as day (day)}
							<tr>
								<th scope="row">{fmtDay(day, true)}</th>
								{#each series as s (s.retailer)}
									{@const p = s.points.find(pt => pt.day === day)}
									<td>{p ? fmt(p.price) : '—'}</td>
								{/each}
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<div class="ph-chart" bind:clientWidth={width}>
				{#if width > 0}
					<svg
						width={width}
						height={HEIGHT}
						role="slider"
						aria-label="Price history by date"
						aria-valuemin={0}
						aria-valuemax={days.length - 1}
						aria-valuenow={hoverIdx ?? days.length - 1}
						aria-valuetext={valueText}
						tabindex="0"
						onpointermove={onPointer}
						onpointerleave={() => (hoverIdx = null)}
						onkeydown={onKey}
						onfocus={() => (hoverIdx ??= days.length - 1)}
						onblur={() => (hoverIdx = null)}
					>
						<!-- Grid + y axis -->
						{#each yDomain.ticks as v (v)}
							<line class="grid" x1={M.left} x2={plotRight} y1={y(v)} y2={y(v)} />
							<text class="tick" x={M.left - 8} y={y(v)} text-anchor="end" dominant-baseline="middle">{fmt(v)}</text>
						{/each}
						<!-- x axis -->
						{#each xTicks as day (day)}
							<text class="tick" x={x(dayTime(day))} y={HEIGHT - 6} text-anchor="middle">{fmtDay(day)}</text>
						{/each}

						{#if series.length === 1 && series[0].points.length > 1}
							<path d={areaPath(series[0])} class="area s{series[0].slot}" />
						{/if}
						{#each series as s (s.retailer)}
							{#if s.points.length > 1}
								<path d={linePath(s)} class="line s{s.slot}" />
							{/if}
						{/each}

						{#if hoverDay}
							<line class="crosshair" x1={x(dayTime(hoverDay))} x2={x(dayTime(hoverDay))} y1={M.top} y2={M.top + plotH} />
							{#each hoverRows as { s, p } (s.retailer)}
								{@render marker(s.slot, x(p.t), y(p.price))}
							{/each}
						{:else}
							{#each series as s (s.retailer)}
								{@const last = s.points[s.points.length - 1]}
								{@render marker(s.slot, x(last.t), y(last.price))}
							{/each}
						{/if}

						{#each endLabels as l (l.retailer)}
							<text class="end-label" x={plotRight + LABEL_GAP} y={l.y} dominant-baseline="middle">{l.text}</text>
						{/each}
					</svg>

					{#if hoverDay && hoverRows.length}
						<div
							class="ph-tooltip"
							style="left: {tooltipFlip ? tooltipLeft - 12 : tooltipLeft + 12}px; transform: translateX({tooltipFlip ? '-100%' : '0'});"
							aria-live="polite"
						>
							<div class="tt-date">{fmtDay(hoverDay, true)}</div>
							{#each hoverRows as { s, p } (s.retailer)}
								<div class="tt-row">
									{@render key(s.slot)}
									<span class="tt-value">{fmt(p.price)}</span>
									<span class="tt-name">{retailerLabel(s.retailer)}</span>
								</div>
							{/each}
						</div>
					{/if}
				{/if}
			</div>
		{/if}

		{#if overflowRetailers.length}
			<div class="ph-note">Also listed by {overflowRetailers.map(retailerLabel).join(', ')} (not charted).</div>
		{/if}
	</section>
{/if}

<style>
	.price-history {
		/* Series colours from the theme, falling back for themes without them. */
		--s0: var(--chart-1, var(--accent));
		--s1: var(--chart-2, var(--info));
		--s2: var(--chart-3, var(--text2));
		margin-top: 18px;
		padding-top: 14px;
		border-top: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.ph-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: 12px;
		flex-wrap: wrap;
	}

	h4 {
		margin: 0;
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text2);
	}

	.ph-sub {
		margin-top: 3px;
		font-size: 0.8rem;
		color: var(--text2);
	}

	.ph-change {
		color: var(--text);
		font-weight: 600;
	}

	.ph-controls {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}

	.seg {
		display: inline-flex;
		border: 1px solid var(--border2);
		border-radius: var(--radius, 6px);
		overflow: hidden;
	}

	.seg button {
		background: none;
		border: none;
		color: var(--text2);
		font: inherit;
		font-size: 0.75rem;
		padding: 3px 10px;
		cursor: pointer;
	}

	.seg button + button {
		border-left: 1px solid var(--border2);
	}

	.seg button.active {
		background: var(--surface2);
		color: var(--text);
		font-weight: 600;
	}

	.ph-legend {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 14px;
	}

	.legend-item {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		background: none;
		border: none;
		padding: 2px 0;
		color: var(--text2);
		font: inherit;
		font-size: 0.8rem;
		cursor: pointer;
	}

	.legend-item.off {
		opacity: 0.45;
	}

	.legend-item.off .legend-name {
		text-decoration: line-through;
	}

	.legend-value {
		color: var(--text);
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}

	.ph-chart {
		position: relative;
		width: 100%;
		min-height: 220px;
	}

	.ph-chart svg {
		display: block;
		overflow: visible;
		outline: none;
		touch-action: pan-y;
	}

	.ph-chart svg:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 4px;
		border-radius: 4px;
	}

	.grid {
		stroke: var(--border);
		stroke-width: 1;
		shape-rendering: crispEdges;
	}

	.crosshair {
		stroke: var(--text3);
		stroke-width: 1;
		shape-rendering: crispEdges;
	}

	.tick {
		fill: var(--text2);
		font-size: 11px;
		font-variant-numeric: tabular-nums;
	}

	.end-label {
		fill: var(--text);
		font-size: 12px;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}

	.line {
		fill: none;
		stroke-width: 2;
		stroke-linejoin: round;
		stroke-linecap: round;
	}

	.area { stroke: none; opacity: 0.1; }

	.mark {
		stroke: var(--bg2);
		stroke-width: 2;
		paint-order: stroke;
	}

	.line.s0 { stroke: var(--s0); }
	.line.s1 { stroke: var(--s1); }
	.line.s2 { stroke: var(--s2); }
	.mark.s0, .area.s0 { fill: var(--s0); }
	.mark.s1, .area.s1 { fill: var(--s1); }
	.mark.s2, .area.s2 { fill: var(--s2); }

	.key { flex-shrink: 0; overflow: visible; }

	.ph-tooltip {
		position: absolute;
		top: 4px;
		pointer-events: none;
		background: var(--surface);
		border: 1px solid var(--border2);
		border-radius: var(--radius, 6px);
		box-shadow: var(--shadow-sm);
		padding: 7px 10px;
		font-size: 0.8rem;
		white-space: nowrap;
		z-index: 2;
	}

	.ph-tooltip .mark { stroke: var(--surface); }

	.tt-date {
		color: var(--text2);
		font-size: 0.72rem;
		margin-bottom: 4px;
	}

	.tt-row {
		display: flex;
		align-items: center;
		gap: 7px;
	}

	.tt-value {
		color: var(--text);
		font-weight: 700;
		font-variant-numeric: tabular-nums;
	}

	.tt-name { color: var(--text2); }

	.ph-table-wrap {
		max-height: 240px;
		overflow: auto;
		border: 1px solid var(--border);
		border-radius: var(--radius, 6px);
	}

	.ph-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.8rem;
		font-variant-numeric: tabular-nums;
	}

	.ph-table th, .ph-table td {
		padding: 5px 10px;
		text-align: right;
		border-bottom: 1px solid var(--border);
	}

	.ph-table th:first-child { text-align: left; }

	.ph-table thead th {
		position: sticky;
		top: 0;
		background: var(--surface);
		color: var(--text2);
		font-weight: 600;
	}

	.ph-table tbody th { font-weight: 400; color: var(--text2); }

	.ph-note {
		font-size: 0.75rem;
		color: var(--text3);
	}
</style>
