<script lang="ts">
	// Line chart of a collection's value over time, in one currency (never two
	// y-axes — the page switches currency instead). Same look and hover/keyboard
	// behaviour as PriceHistoryChart.
	import type { components } from '$lib/generated/api';
	import { formatMoney } from '$lib/currency.svelte';

	type Point = components['schemas']['ValueHistoryPoint'];

	interface Props {
		points: Point[];
		currency: string;
		/** Owned entries valued in this currency, for "N of M priced". */
		entryCount: number;
		height?: number;
		/** Shows the data as a table instead; the page owns the toggle. */
		showTable?: boolean;
	}

	let { points, currency, entryCount, height = 280, showTable = false }: Props = $props();

	const M = { top: 14, bottom: 26, left: 72, right: 16 };

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
		const values = points.map(p => p.value);
		let lo = Math.min(...values), hi = Math.max(...values);
		if (lo === hi) { const pad = Math.abs(lo) * 0.1 || 1; lo -= pad; hi += pad; }
		const step = niceStep((hi - lo) / 4);
		lo = Math.max(0, Math.floor(lo / step) * step);
		hi = Math.ceil(hi / step) * step;
		const ticks: number[] = [];
		for (let v = lo; v <= hi + step / 2; v += step) ticks.push(+v.toFixed(10));
		return { lo, hi, ticks };
	});

	const tFirst = $derived(points.length ? dayTime(points[0].day) : 0);
	const tLast = $derived(points.length ? dayTime(points[points.length - 1].day) : 0);
	const plotW = $derived(Math.max(10, width - M.left - M.right));
	const plotH = $derived(height - M.top - M.bottom);

	const x = (t: number) => (tLast === tFirst ? M.left + plotW / 2 : M.left + ((t - tFirst) / (tLast - tFirst)) * plotW);
	const y = (v: number) => M.top + plotH - ((v - yDomain.lo) / (yDomain.hi - yDomain.lo)) * plotH;

	const spanDays = $derived((tLast - tFirst) / 86_400_000);
	const xTicks = $derived.by(() => {
		if (!points.length) return [];
		const max = Math.max(2, Math.floor(plotW / 90));
		if (points.length <= max) return points.map(p => p.day);
		const picked = new Set<string>();
		for (let i = 0; i < max; i++) picked.add(points[Math.round((i * (points.length - 1)) / (max - 1))].day);
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

	const fmt = (v: number) => formatMoney(v, currency);

	const linePath = $derived(points.map((p, i) => `${i ? 'L' : 'M'}${x(dayTime(p.day)).toFixed(1)},${y(p.value).toFixed(1)}`).join(''));
	const areaPath = $derived.by(() => {
		if (points.length < 2) return '';
		const base = (M.top + plotH).toFixed(1);
		return `${linePath}L${x(tLast).toFixed(1)},${base}L${x(tFirst).toFixed(1)},${base}Z`;
	});

	// ── Hover / keyboard ────────────────────────────────────────────────────
	let hoverIdx = $state<number | null>(null);
	const hover = $derived(hoverIdx !== null ? points[hoverIdx] ?? null : null);

	function nearestIdx(px: number): number {
		let best = 0;
		for (let i = 1; i < points.length; i++) {
			if (Math.abs(x(dayTime(points[i].day)) - px) < Math.abs(x(dayTime(points[best].day)) - px)) best = i;
		}
		return best;
	}

	function onPointer(e: PointerEvent) {
		const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
		hoverIdx = nearestIdx(e.clientX - rect.left);
	}

	function onKey(e: KeyboardEvent) {
		if (!points.length) return;
		if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
			e.preventDefault();
			const cur = hoverIdx ?? points.length - 1;
			hoverIdx = Math.min(points.length - 1, Math.max(0, cur + (e.key === 'ArrowLeft' ? -1 : 1)));
		} else if (e.key === 'Home' || e.key === 'End') {
			e.preventDefault();
			hoverIdx = e.key === 'Home' ? 0 : points.length - 1;
		}
	}

	const valueText = $derived.by(() => {
		const p = hover ?? points[points.length - 1];
		return p ? `${fmtDay(p.day, true)}: ${fmt(p.value)}` : '';
	});

	const tooltipLeft = $derived(hover ? x(dayTime(hover.day)) : 0);
	const tooltipFlip = $derived(tooltipLeft > width - 200);
</script>

<div class="vh">
	{#if showTable}
		<div class="vh-table-wrap">
			<table class="vh-table">
				<thead>
					<tr><th scope="col">Date</th><th scope="col">Value</th><th scope="col">Priced</th></tr>
				</thead>
				<tbody>
					{#each [...points].reverse() as p (p.day)}
						<tr>
							<th scope="row">{fmtDay(p.day, true)}</th>
							<td>{fmt(p.value)}</td>
							<td>{p.priced_count} / {entryCount}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else}
		<div class="vh-chart" style="min-height: {height}px" bind:clientWidth={width}>
			{#if width > 0 && points.length}
				<svg
					{width}
					{height}
					role="slider"
					aria-label="Collection value by date"
					aria-valuemin={0}
					aria-valuemax={points.length - 1}
					aria-valuenow={hoverIdx ?? points.length - 1}
					aria-valuetext={valueText}
					tabindex="0"
					onpointermove={onPointer}
					onpointerleave={() => (hoverIdx = null)}
					onkeydown={onKey}
					onfocus={() => (hoverIdx ??= points.length - 1)}
					onblur={() => (hoverIdx = null)}
				>
					{#each yDomain.ticks as v (v)}
						<line class="grid" x1={M.left} x2={M.left + plotW} y1={y(v)} y2={y(v)} />
						<text class="tick" x={M.left - 8} y={y(v)} text-anchor="end" dominant-baseline="middle">{fmt(v)}</text>
					{/each}
					{#each xTicks as day (day)}
						<text class="tick" x={x(dayTime(day))} y={height - 6} text-anchor="middle">{fmtDay(day)}</text>
					{/each}

					{#if areaPath}<path d={areaPath} class="area" />{/if}
					{#if points.length > 1}<path d={linePath} class="line" />{/if}

					{#if hover}
						<line class="crosshair" x1={x(dayTime(hover.day))} x2={x(dayTime(hover.day))} y1={M.top} y2={M.top + plotH} />
						<circle class="mark" cx={x(dayTime(hover.day))} cy={y(hover.value)} r="4.5" />
					{:else}
						{@const last = points[points.length - 1]}
						<circle class="mark" cx={x(dayTime(last.day))} cy={y(last.value)} r="4.5" />
					{/if}
				</svg>

				{#if hover}
					<div
						class="vh-tooltip"
						style="left: {tooltipFlip ? tooltipLeft - 12 : tooltipLeft + 12}px; transform: translateX({tooltipFlip ? '-100%' : '0'});"
						aria-live="polite"
					>
						<div class="tt-date">{fmtDay(hover.day, true)}</div>
						<div class="tt-value">{fmt(hover.value)}</div>
						{#if hover.priced_count < entryCount}
							<div class="tt-note">{hover.priced_count} of {entryCount} entries priced</div>
						{/if}
					</div>
				{/if}
			{/if}
		</div>
	{/if}
</div>

<style>
	.vh {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.vh-chart {
		position: relative;
		width: 100%;
	}

	.vh-chart svg {
		display: block;
		overflow: visible;
		outline: none;
		touch-action: pan-y;
	}

	.vh-chart svg:focus-visible {
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

	.line {
		fill: none;
		stroke: var(--chart-1, var(--accent));
		stroke-width: 2;
		stroke-linejoin: round;
		stroke-linecap: round;
	}

	.area {
		fill: var(--chart-1, var(--accent));
		stroke: none;
		opacity: 0.1;
	}

	.mark {
		fill: var(--chart-1, var(--accent));
		stroke: var(--surface);
		stroke-width: 2;
		paint-order: stroke;
	}

	.vh-tooltip {
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

	.tt-date { color: var(--text2); font-size: 0.72rem; margin-bottom: 2px; }
	.tt-value { color: var(--text); font-weight: 700; font-variant-numeric: tabular-nums; }
	.tt-note { color: var(--text3); font-size: 0.72rem; margin-top: 2px; }

	.vh-table-wrap {
		max-height: 320px;
		overflow: auto;
		border: 1px solid var(--border);
		border-radius: var(--radius, 6px);
	}

	.vh-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.8rem;
		font-variant-numeric: tabular-nums;
	}

	.vh-table th, .vh-table td {
		padding: 5px 10px;
		text-align: right;
		border-bottom: 1px solid var(--border);
	}

	.vh-table th:first-child { text-align: left; }

	.vh-table thead th {
		position: sticky;
		top: 0;
		background: var(--surface);
		color: var(--text2);
		font-weight: 600;
	}

	.vh-table tbody th { font-weight: 400; color: var(--text2); }
</style>
