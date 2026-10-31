<script lang="ts">
	// Line chart of a collection's value over time, a line per currency on one
	// shared y-axis (never two). Same look and hover/keyboard behaviour as
	// PriceHistoryChart, including its limit of three series: the theme's
	// `--chart-N` palettes are validated for no more.
	import type { components } from '$lib/generated/api';
	import { formatMoney } from '$lib/currency.svelte';

	type Point = components['schemas']['ValueHistoryPoint'];
	type Series = components['schemas']['CurrencyValueHistory'];

	interface Props {
		/** One per currency; a currency's position here is its colour slot. */
		series: Series[];
		height?: number;
		/** Shows the data as a table instead; the page owns the toggle. */
		showTable?: boolean;
	}

	let { series, height = 280, showTable = false }: Props = $props();

	const MAX_SERIES = 3;
	const M = { top: 14, bottom: 26, left: 72, right: 16 };

	let width = $state(0);

	const lines = $derived(series.map((s, slot) => ({ ...s, slot })).filter(s => s.slot < MAX_SERIES && s.points.length));
	const overflow = $derived(series.slice(MAX_SERIES).filter(s => s.points.length).map(s => s.currency));

	let hidden = $state<Set<string>>(new Set());
	const visible = $derived.by(() => {
		const shown = lines.filter(s => !hidden.has(s.currency));
		return shown.length ? shown : lines;
	});

	function toggleLine(currency: string) {
		const next = new Set(hidden);
		if (next.has(currency)) next.delete(currency);
		// Never hide the last visible line.
		else if (lines.filter(s => !next.has(s.currency)).length > 1) next.add(currency);
		hidden = next;
	}

	const days = $derived([...new Set(visible.flatMap(s => s.points.map(p => p.day)))].sort());

	function dayTime(day: string): number {
		return Date.parse(`${day.slice(0, 10)}T00:00:00Z`);
	}

	function niceStep(raw: number): number {
		const pow = 10 ** Math.floor(Math.log10(raw));
		const n = raw / pow;
		return (n <= 1 ? 1 : n <= 2 ? 2 : n <= 2.5 ? 2.5 : n <= 5 ? 5 : 10) * pow;
	}

	const yDomain = $derived.by(() => {
		const values = visible.flatMap(s => s.points.map(p => p.value));
		let lo = Math.min(...values), hi = Math.max(...values);
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
	const plotW = $derived(Math.max(10, width - M.left - M.right));
	const plotH = $derived(height - M.top - M.bottom);

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

	// Lines in different currencies share the axis, so its ticks can't name one.
	function fmtTick(v: number): string {
		return visible.length === 1
			? formatMoney(v, visible[0].currency)
			: v.toLocaleString('en', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
	}

	function linePath(s: Series): string {
		return s.points.map((p, i) => `${i ? 'L' : 'M'}${x(dayTime(p.day)).toFixed(1)},${y(p.value).toFixed(1)}`).join('');
	}

	function areaPath(s: Series): string {
		const base = (M.top + plotH).toFixed(1);
		const first = s.points[0], last = s.points[s.points.length - 1];
		return `${linePath(s)}L${x(dayTime(last.day)).toFixed(1)},${base}L${x(dayTime(first.day)).toFixed(1)},${base}Z`;
	}

	/** The value on `day`: as with the server's series, the last one carries forward. */
	function pointAt(s: Series, day: string): Point | undefined {
		return s.points.findLast(p => p.day <= day);
	}

	// ── Hover / keyboard ────────────────────────────────────────────────────
	let hoverIdx = $state<number | null>(null);
	const hoverDay = $derived(hoverIdx !== null ? days[hoverIdx] ?? null : null);
	const hoverRows = $derived(
		hoverDay
			? visible.flatMap(s => {
				const p = pointAt(s, hoverDay);
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

	const valueText = $derived.by(() => {
		const day = hoverDay ?? days[days.length - 1];
		if (!day) return '';
		const values = visible.flatMap(s => {
			const p = pointAt(s, day);
			return p ? [formatMoney(p.value, s.currency)] : [];
		});
		return `${fmtDay(day, true)}: ${values.join(', ')}`;
	});

	const tooltipLeft = $derived(hoverDay ? x(dayTime(hoverDay)) : 0);
	const tooltipFlip = $derived(tooltipLeft > width - 200);
</script>

{#snippet marker(slot: number, cx: number, cy: number, r = 4.5)}
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

<div class="vh">
	{#if lines.length > 1}
		<div class="vh-legend">
			{#each lines as s (s.currency)}
				<button
					class="legend-item"
					class:off={!visible.includes(s)}
					aria-pressed={visible.includes(s)}
					title={visible.includes(s) ? 'Hide' : 'Show'}
					onclick={() => toggleLine(s.currency)}
				>
					{@render key(s.slot)}
					<span>{s.currency}</span>
					<span class="legend-value">{formatMoney(s.points[s.points.length - 1].value, s.currency)}</span>
				</button>
			{/each}
		</div>
	{/if}

	{#if showTable}
		<div class="vh-table-wrap">
			<table class="vh-table">
				<thead>
					<tr>
						<th scope="col">Date</th>
						{#each visible as s (s.currency)}
							<th scope="col">{visible.length > 1 ? s.currency : 'Value'}</th>
							<th scope="col">{visible.length > 1 ? `${s.currency} priced` : 'Priced'}</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each [...days].reverse() as day (day)}
						<tr>
							<th scope="row">{fmtDay(day, true)}</th>
							{#each visible as s (s.currency)}
								{@const p = pointAt(s, day)}
								<td>{p ? formatMoney(p.value, s.currency) : '—'}</td>
								<td>{p ? `${p.priced_count} / ${s.entry_count}` : '—'}</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else}
		<div class="vh-chart" style="min-height: {height}px" bind:clientWidth={width}>
			{#if width > 0 && days.length}
				<svg
					{width}
					{height}
					role="slider"
					aria-label="Collection value by date"
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
					{#each yDomain.ticks as v (v)}
						<line class="grid" x1={M.left} x2={M.left + plotW} y1={y(v)} y2={y(v)} />
						<text class="tick" x={M.left - 8} y={y(v)} text-anchor="end" dominant-baseline="middle">{fmtTick(v)}</text>
					{/each}
					{#each xTicks as day (day)}
						<text class="tick" x={x(dayTime(day))} y={height - 6} text-anchor="middle">{fmtDay(day)}</text>
					{/each}

					{#if visible.length === 1 && visible[0].points.length > 1}
						<path d={areaPath(visible[0])} class="area s{visible[0].slot}" />
					{/if}
					{#each visible as s (s.currency)}
						{#if s.points.length > 1}<path d={linePath(s)} class="line s{s.slot}" />{/if}
					{/each}

					{#if hoverDay}
						<line class="crosshair" x1={x(dayTime(hoverDay))} x2={x(dayTime(hoverDay))} y1={M.top} y2={M.top + plotH} />
						{#each hoverRows as { s, p } (s.currency)}
							{@render marker(s.slot, x(dayTime(hoverDay)), y(p.value))}
						{/each}
					{:else}
						{#each visible as s (s.currency)}
							{@const last = s.points[s.points.length - 1]}
							{@render marker(s.slot, x(dayTime(last.day)), y(last.value))}
						{/each}
					{/if}
				</svg>

				{#if hoverDay && hoverRows.length}
					<div
						class="vh-tooltip"
						style="left: {tooltipFlip ? tooltipLeft - 12 : tooltipLeft + 12}px; transform: translateX({tooltipFlip ? '-100%' : '0'});"
						aria-live="polite"
					>
						<div class="tt-date">{fmtDay(hoverDay, true)}</div>
						{#each hoverRows as { s, p } (s.currency)}
							<div class="tt-row">
								{#if hoverRows.length > 1}{@render key(s.slot)}{/if}
								<span class="tt-value">{formatMoney(p.value, s.currency)}</span>
							</div>
							{#if p.priced_count < s.entry_count}
								<div class="tt-note">{p.priced_count} of {s.entry_count} entries priced</div>
							{/if}
						{/each}
					</div>
				{/if}
			{/if}
		</div>
	{/if}

	{#if overflow.length}
		<div class="tt-note">Also valued in {overflow.join(', ')} (not charted).</div>
	{/if}
</div>

<style>
	.vh {
		/* Series colours from the theme, falling back for themes without them. */
		--s0: var(--chart-1, var(--accent));
		--s1: var(--chart-2, var(--info));
		--s2: var(--chart-3, var(--text2));
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
		stroke-width: 2;
		stroke-linejoin: round;
		stroke-linecap: round;
	}

	.area {
		stroke: none;
		opacity: 0.1;
	}

	.mark {
		stroke: var(--surface);
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

	.vh-legend {
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

	.legend-item.off { opacity: 0.45; }
	.legend-item.off span:not(.legend-value) { text-decoration: line-through; }

	.legend-value {
		color: var(--text);
		font-weight: 600;
		font-variant-numeric: tabular-nums;
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
	.tt-row { display: flex; align-items: center; gap: 7px; }
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
