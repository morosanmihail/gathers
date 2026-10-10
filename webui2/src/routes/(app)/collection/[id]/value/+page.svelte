<script lang="ts">
	// Everything about a collection's value: current totals and profit, the
	// value of what's held now over time (from price history), and which
	// cards make up, earned or moved that value.
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import {
		getCollectionValue, getCollectionValueHistory, getCollectionValueCards,
		type CollectionValueHistory, type CollectionValueCards, type ValueCardEntry
	} from '$lib/api';
	import { app } from '$lib/state.svelte';
	import type { ValueBreakdown, CollectionCard } from '$lib/types';
	import { finishLabel } from '$lib/types';
	import {
		formatMoney, formatMoneyList, preferredCurrency, sumByCurrency,
		convertTotal, convertTotalsEnabled, getRates, type Money
	} from '$lib/currency.svelte';
	import ConvertedTotal from '$lib/components/ConvertedTotal.svelte';
	import CardImageTooltip from '$lib/components/CardImageTooltip.svelte';
	import ValueHistoryChart from '$lib/components/ValueHistoryChart.svelte';

	const collectionId = $derived(decodeURIComponent($page.params.id ?? ''));
	const collectionHref = $derived(`/collection/${encodeURIComponent(collectionId)}`);

	const TOP_N = 10;
	const RANGES = [
		{ label: '1M', days: 30 },
		{ label: '3M', days: 90 },
		{ label: '6M', days: 182 },
		{ label: '1Y', days: 365 },
		{ label: 'All', days: null }
	] as const;
	type Range = (typeof RANGES)[number];

	let breakdown = $state<ValueBreakdown | null>(null);
	let history = $state<CollectionValueHistory | null>(null);
	let cards = $state<CollectionValueCards | null>(null);
	let loading = $state(true);
	let rangeKey = $state<Range['label']>('All');
	const range = $derived(RANGES.find(r => r.label === rangeKey) ?? RANGES[RANGES.length - 1]);
	let rates = $state<Record<string, number> | null>(null);

	$effect(() => {
		if (app.ready && (!app.collectionsEnabled || !app.pricingEnabled)) {
			goto(app.collectionsEnabled ? collectionHref : '/search', { replaceState: true });
		}
	});

	$effect(() => {
		const id = collectionId;
		if (!app.pricingEnabled) return;
		loading = true;
		Promise.all([getCollectionValue(id), getCollectionValueHistory(id)]).then(([b, h]) => {
			if (id !== collectionId) return;
			breakdown = b;
			history = h;
			loading = false;
		});
	});

	// "All" reads each card's first recorded price, so any window works.
	const cardDays = $derived(range.days ?? 30);
	$effect(() => {
		const id = collectionId, days = cardDays;
		if (!app.pricingEnabled) return;
		getCollectionValueCards(id, days).then(c => {
			if (id === collectionId && days === cardDays) cards = c;
		});
	});

	$effect(() => {
		if (convertTotalsEnabled() && !rates) getRates().then(r => { rates = r; });
	});

	// ── Cross-currency helpers ──────────────────────────────────────────────
	const target = $derived(preferredCurrency());

	/** `m` in the preferred currency, when it is or rates allow. */
	function converted(m: Money): number | null {
		if (m.currency === target) return m.value;
		return rates ? convertTotal([m], target, rates) : null;
	}

	/** Comparable amount for ranking; unconverted when no rates. */
	function rankValue(m: Money): number {
		return converted(m) ?? m.value;
	}

	// ── Totals ──────────────────────────────────────────────────────────────
	const currencyTotals = $derived((breakdown?.currencies ?? []).map(c => ({ currency: c.currency, value: c.total_value })));
	const profitTotals = $derived((breakdown?.currencies ?? []).map(c => ({ currency: c.currency, value: c.profit })));
	const untrackedTotals = $derived((breakdown?.currencies ?? []).map(c => ({ currency: c.currency, value: c.untracked_value })));
	const wantedTotals = $derived((breakdown?.currencies ?? []).map(c => ({ currency: c.currency, value: c.wanted_value })));
	const grandTotal = $derived(rates ? convertTotal(currencyTotals, target, rates) : null);

	// ── Per-card ────────────────────────────────────────────────────────────
	const entries = $derived(cards?.entries ?? []);

	/** Price to compare against for the selected range. */
	function refPrice(e: ValueCardEntry) {
		return range.days === null ? e.first_price : e.past_price;
	}

	/** Value change of the copies held, over the range, in the entry's currency. */
	function movement(e: ValueCardEntry): number | null {
		const ref = refPrice(e);
		return ref ? (e.unit_price - ref.price) * e.quantity : null;
	}

	/** Profit as `Money`: the server's, or converted when bought in another currency. */
	function profitOf(e: ValueCardEntry): (Money & { approx: boolean }) | null {
		if (e.profit != null) return { currency: e.currency, value: e.profit, approx: false };
		if (!e.cost_quantity || !rates) return null;
		const value = converted({ currency: e.currency, value: e.unit_price * e.cost_quantity });
		const cost = convertTotal(e.cost, target, rates);
		return value != null && cost != null ? { currency: target, value: value - cost, approx: true } : null;
	}

	const mostValuable = $derived(
		[...entries].sort((a, b) => rankValue({ currency: b.currency, value: b.total_value }) - rankValue({ currency: a.currency, value: a.total_value })).slice(0, TOP_N)
	);
	const topValueShare = $derived.by(() => {
		const all = entries.reduce((s, e) => s + rankValue({ currency: e.currency, value: e.total_value }), 0);
		const top = mostValuable.reduce((s, e) => s + rankValue({ currency: e.currency, value: e.total_value }), 0);
		return all > 0 ? (top / all) * 100 : null;
	});
	const maxTopValue = $derived(Math.max(0, ...mostValuable.map(e => rankValue({ currency: e.currency, value: e.total_value }))));

	const withProfit = $derived(entries.flatMap(e => {
		const p = profitOf(e);
		return p ? [{ e, p }] : [];
	}));
	const topProfits = $derived(withProfit.filter(x => x.p.value > 0).sort((a, b) => rankValue(b.p) - rankValue(a.p)).slice(0, TOP_N));
	const topLosses = $derived(withProfit.filter(x => x.p.value < 0).sort((a, b) => rankValue(a.p) - rankValue(b.p)).slice(0, TOP_N));

	const withMovement = $derived(entries.flatMap(e => {
		const m = movement(e);
		return m != null && m !== 0 ? [{ e, m: { currency: e.currency, value: m } }] : [];
	}));
	const risers = $derived(withMovement.filter(x => x.m.value > 0).sort((a, b) => rankValue(b.m) - rankValue(a.m)).slice(0, TOP_N));
	const fallers = $derived(withMovement.filter(x => x.m.value < 0).sort((a, b) => rankValue(a.m) - rankValue(b.m)).slice(0, TOP_N));

	/** Like-for-like change over the range: only entries priced at both ends. */
	const marketMove = $derived.by(() => {
		const tracked = entries.filter(e => refPrice(e));
		const change = sumByCurrency(tracked.map(e => ({ currency: e.currency, value: movement(e) ?? 0 })));
		const base = sumByCurrency(tracked.map(e => ({ currency: e.currency, value: refPrice(e)!.price * e.quantity })));
		const conv = (items: Money[]) => (rates ? convertTotal(items, target, rates) : null);
		const c = conv(change), b = conv(base);
		// Percent only when it's well defined: one currency, or all converted.
		const pct = c != null && b ? (c / b) * 100
			: change.length === 1 && base[0]?.value ? (change[0].value / base[0].value) * 100
			: null;
		return { change, pct, tracked: tracked.length };
	});

	const costTotals = $derived(sumByCurrency(entries.flatMap(e => e.cost)));

	const GAME_LABELS: Record<string, string> = {
		MagicSQLite: 'Magic', Scryfall: 'Magic', PokemonSQLite: 'Pokémon', RiftboundSQLite: 'Riftbound'
	};
	const gameLabel = (provider: string) => GAME_LABELS[provider] ?? provider.replace(/^plugin-/, '');

	const byGame = $derived.by(() => {
		const groups = new Map<string, { label: string; items: Money[]; count: number }>();
		for (const e of entries) {
			const label = gameLabel(e.provider || 'MagicSQLite');
			const g = groups.get(label) ?? { label, items: [], count: 0 };
			g.items.push({ currency: e.currency, value: e.total_value });
			g.count += 1;
			groups.set(label, g);
		}
		const list = [...groups.values()].map(g => ({ ...g, items: sumByCurrency(g.items), rank: g.items.reduce((s, m) => s + rankValue(m), 0) }));
		const all = list.reduce((s, g) => s + g.rank, 0);
		return list.sort((a, b) => b.rank - a.rank).map(g => ({ ...g, share: all > 0 ? (g.rank / all) * 100 : 0 }));
	});

	// ── History ─────────────────────────────────────────────────────────────
	const series = $derived(history?.currencies ?? []);
	let chosenCurrency = $state<string | null>(null);
	const current = $derived(
		series.find(s => s.currency === chosenCurrency) ?? series.find(s => s.currency === target) ?? series[0] ?? null
	);
	const rangeStart = $derived.by(() => {
		if (range.days === null) return '';
		const d = new Date();
		d.setUTCDate(d.getUTCDate() - range.days);
		return d.toISOString().slice(0, 10);
	});
	const points = $derived((current?.points ?? []).filter(p => p.day >= rangeStart));
	const seriesChange = $derived.by(() => {
		if (points.length < 2) return null;
		const first = points[0], last = points[points.length - 1];
		return { abs: last.value - first.value, pct: first.value > 0 ? ((last.value - first.value) / first.value) * 100 : null, sameEntries: first.priced_count === last.priced_count };
	});
	let showTable = $state(false);

	// ── Formatting ──────────────────────────────────────────────────────────
	function signed(m: Money, approx = false): string {
		return `${approx ? '≈ ' : ''}${m.value >= 0 ? '+' : '−'}${formatMoney(Math.abs(m.value), m.currency)}`;
	}

	function signedList(items: Money[]): string {
		const nonZero = items.filter(m => m.value !== 0);
		return nonZero.length ? nonZero.map(m => signed(m)).join(' ') : formatMoney(0, target);
	}

	function pct(v: number | null): string {
		return v == null || !isFinite(v) ? '' : `${v >= 0 ? '▲' : '▼'} ${Math.abs(v).toFixed(1)}%`;
	}

	function fmtDay(day: string): string {
		return new Date(`${day}T00:00:00Z`).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric', timeZone: 'UTC' });
	}

	function tone(v: number): string {
		return v > 0 ? 'pos' : v < 0 ? 'neg' : '';
	}

	function asCard(e: ValueCardEntry): CollectionCard {
		return { name: e.name ?? e.card_uuid, image: e.image ?? undefined, cardIdentifiers: e.scryfall_id ? { scryfallId: e.scryfall_id } : undefined } as unknown as CollectionCard;
	}

	const rangeLabel = $derived(range.days === null ? 'since tracking began' : `over ${range.label}`);
</script>

<svelte:head>
	<title>{collectionId} value - gatheRs</title>
</svelte:head>

{#snippet cardCell(e: ValueCardEntry)}
	<div class="card-cell">
		<span class="card-name"><CardImageTooltip card={asCard(e)} /></span>
		<span class="card-meta">
			{[e.set_code, e.finish ? finishLabel(e.finish) : null, `×${e.quantity}`].filter(Boolean).join(' · ')}
		</span>
	</div>
{/snippet}

{#snippet emptyNote(text: string)}
	<div class="list-empty">{text}</div>
{/snippet}

<div class="value-page">
	<div class="page-header">
		<a class="back" href={collectionHref}>← {collectionId}</a>
	</div>
	<div class="page-header title-row">
		<h1 class="page-title">Collection value</h1>
		<div class="seg range-seg" role="group" aria-label="Time range">
			{#each RANGES as r (r.label)}
				<button class:active={r.label === rangeKey} aria-pressed={r.label === rangeKey} onclick={() => (rangeKey = r.label)}>{r.label}</button>
			{/each}
		</div>
	</div>

	{#if loading}
		<div class="loading-row"><div class="spinner"></div> Loading…</div>
	{:else if !breakdown}
		<div class="empty-state"><div class="empty-state-text">Couldn't load this collection's value.</div></div>
	{:else}
		<!-- ── KPI tiles ───────────────────────────────────────────────── -->
		<div class="kpis">
			<div class="stat-card">
				<span class="stat-label">Current value</span>
				<span class="kpi-value">{formatMoneyList(currencyTotals) || formatMoney(0, target)}</span>
				<span class="stat-sub">
					{#if grandTotal != null && currencyTotals.filter(m => m.value).length > 1}
						= {formatMoney(grandTotal, target)} ·
					{/if}
					{breakdown.priced_count} of {breakdown.total_count} entries priced
				</span>
			</div>
			<div class="stat-card">
				<span class="stat-label">Change {rangeLabel}</span>
				{#if cards && marketMove.tracked > 0}
					<span class="kpi-value {marketMove.change.length === 1 ? tone(marketMove.change[0].value) : ''}">{signedList(marketMove.change)}</span>
					<span class="stat-sub">
						{pct(marketMove.pct)}{marketMove.pct != null ? ' · ' : ''}price moves of {marketMove.tracked} entr{marketMove.tracked === 1 ? 'y' : 'ies'} priced then and now
					</span>
				{:else}
					<span class="kpi-value muted">—</span>
					<span class="stat-sub">{history?.enabled === false ? 'Needs price history' : 'No prices recorded that far back'}</span>
				{/if}
			</div>
			<div class="stat-card">
				<span class="stat-label">Profit</span>
				<span class="kpi-value {profitTotals.length === 1 ? tone(profitTotals[0].value) : ''}">{signedList(profitTotals)}</span>
				<span class="stat-sub">
					{#if rates && profitTotals.filter(m => m.value !== 0).length > 1}<ConvertedTotal items={profitTotals} /> ·{/if}
					{#if costTotals.length}paid {formatMoneyList(costTotals)}{:else}no purchase prices recorded{/if}
					{#if untrackedTotals.some(m => m.value > 0)} · {formatMoneyList(untrackedTotals)} without purchase data{/if}
				</span>
			</div>
			<div class="stat-card">
				<span class="stat-label">Wishlist</span>
				<span class="kpi-value">{formatMoneyList(wantedTotals) || formatMoney(0, target)}</span>
				<span class="stat-sub">to buy every wanted card <ConvertedTotal items={wantedTotals} /></span>
			</div>
		</div>

		<!-- ── History chart ───────────────────────────────────────────── -->
		<section class="panel">
			<div class="panel-header">
				<div>
					<h2>Value over time</h2>
					{#if current && points.length}
						<div class="panel-sub">
							{current.currency} · {points.length} snapshot{points.length === 1 ? '' : 's'} since {fmtDay(points[0].day)}
							{#if seriesChange}
								· <span class="strong">{signed({ currency: current.currency, value: seriesChange.abs })} {pct(seriesChange.pct)}</span>
								{#if !seriesChange.sameEntries}<span class="muted">(includes cards whose prices started being recorded later)</span>{/if}
							{/if}
						</div>
					{/if}
				</div>
				<div class="panel-controls">
					{#if series.length > 1}
						<div class="seg" role="group" aria-label="Currency">
							{#each series as s (s.currency)}
								<button class:active={s.currency === current?.currency} aria-pressed={s.currency === current?.currency} onclick={() => (chosenCurrency = s.currency)}>{s.currency}</button>
							{/each}
						</div>
					{/if}
					{#if points.length}
						<button class="btn btn-ghost btn-sm" onclick={() => (showTable = !showTable)} aria-pressed={showTable}>{showTable ? 'Chart' : 'Table'}</button>
					{/if}
				</div>
			</div>
			{#if history && !history.enabled}
				{@render emptyNote('Price history is off. Turn it on in Settings to track how this collection’s value changes over time.')}
			{:else if !current || !current.points.length}
				{@render emptyNote('No prices recorded for these cards yet. History builds up each time prices update.')}
			{:else if !points.length}
				{@render emptyNote(`No prices recorded in the last ${range.label}.`)}
			{:else}
				<ValueHistoryChart {points} currency={current.currency} entryCount={current.entry_count} {showTable} />
				<p class="note">
					Value of the cards held now, at each day's recorded prices — copies sold or moved away don't count.
					{#if series.length > 1}Each currency is charted separately; cards are valued in their retailer's currency.{/if}
				</p>
			{/if}
		</section>

		<!-- ── Lists ───────────────────────────────────────────────────── -->
		{#if !cards}
			<div class="loading-row"><div class="spinner"></div> Loading cards…</div>
		{:else}
			<div class="lists">
				<section class="panel">
					<div class="panel-header">
						<div>
							<h2>Most valuable</h2>
							{#if topValueShare != null && entries.length > TOP_N}
								<div class="panel-sub">Top {TOP_N} hold {topValueShare.toFixed(0)}% of the value</div>
							{/if}
						</div>
					</div>
					{#if mostValuable.length}
						<ol class="rank">
							{#each mostValuable as e (e.card_uuid + e.finish)}
								<li>
									{@render cardCell(e)}
									<div class="num">
										<span class="strong">{formatMoney(e.total_value, e.currency)}</span>
										{#if e.quantity > 1}<span class="sub">{formatMoney(e.unit_price, e.currency)} each</span>{/if}
									</div>
									<div class="share" aria-hidden="true"><span style="width: {maxTopValue ? (rankValue({ currency: e.currency, value: e.total_value }) / maxTopValue) * 100 : 0}%"></span></div>
								</li>
							{/each}
						</ol>
					{:else}
						{@render emptyNote('No priced cards.')}
					{/if}
				</section>

				<section class="panel">
					<div class="panel-header">
						<div>
							<h2>Highest profits</h2>
							<div class="panel-sub">Current value minus what you paid</div>
						</div>
					</div>
					{#if topProfits.length}
						<ol class="rank">
							{#each topProfits as { e, p } (e.card_uuid + e.finish)}
								<li>
									{@render cardCell(e)}
									<div class="num">
										<span class="strong pos">{signed(p, p.approx)}</span>
										<span class="sub">paid {formatMoneyList(e.cost)} for {e.cost_quantity}</span>
									</div>
								</li>
							{/each}
						</ol>
					{:else}
						{@render emptyNote(withProfit.length ? 'Nothing is worth more than you paid yet.' : 'Record purchase prices when adding cards to see profits here.')}
					{/if}
				</section>

				<section class="panel">
					<div class="panel-header">
						<div>
							<h2>Biggest risers</h2>
							<div class="panel-sub">Value gained {rangeLabel}</div>
						</div>
					</div>
					{#if risers.length}
						<ol class="rank">
							{#each risers as { e, m } (e.card_uuid + e.finish)}
								{@const ref = refPrice(e)!}
								<li>
									{@render cardCell(e)}
									<div class="num">
										<span class="strong pos">{signed(m)}</span>
										<span class="sub">{formatMoney(ref.price, e.currency)} → {formatMoney(e.unit_price, e.currency)} {pct(((e.unit_price - ref.price) / ref.price) * 100)}</span>
									</div>
								</li>
							{/each}
						</ol>
					{:else}
						{@render emptyNote(cards.history_enabled ? `No price rises ${rangeLabel}.` : 'Needs price history.')}
					{/if}
				</section>

				<section class="panel">
					<div class="panel-header">
						<div>
							<h2>Biggest fallers</h2>
							<div class="panel-sub">Value lost {rangeLabel}</div>
						</div>
					</div>
					{#if fallers.length}
						<ol class="rank">
							{#each fallers as { e, m } (e.card_uuid + e.finish)}
								{@const ref = refPrice(e)!}
								<li>
									{@render cardCell(e)}
									<div class="num">
										<span class="strong neg">{signed(m)}</span>
										<span class="sub">{formatMoney(ref.price, e.currency)} → {formatMoney(e.unit_price, e.currency)} {pct(((e.unit_price - ref.price) / ref.price) * 100)}</span>
									</div>
								</li>
							{/each}
						</ol>
					{:else}
						{@render emptyNote(cards.history_enabled ? `No price drops ${rangeLabel}.` : 'Needs price history.')}
					{/if}
				</section>

				{#if topLosses.length}
					<section class="panel">
						<div class="panel-header">
							<div>
								<h2>Below purchase price</h2>
								<div class="panel-sub">Worth less now than you paid</div>
							</div>
						</div>
						<ol class="rank">
							{#each topLosses as { e, p } (e.card_uuid + e.finish)}
								<li>
									{@render cardCell(e)}
									<div class="num">
										<span class="strong neg">{signed(p, p.approx)}</span>
										<span class="sub">paid {formatMoneyList(e.cost)} for {e.cost_quantity}</span>
									</div>
								</li>
							{/each}
						</ol>
					</section>
				{/if}

				{#if byGame.length > 1}
					<section class="panel">
						<div class="panel-header"><div><h2>By game</h2></div></div>
						<ul class="rank">
							{#each byGame as g (g.label)}
								<li>
									<div class="card-cell">
										<span class="card-name">{g.label}</span>
										<span class="card-meta">{g.count} entr{g.count === 1 ? 'y' : 'ies'}</span>
									</div>
									<div class="num">
										<span class="strong">{formatMoneyList(g.items)}</span>
										<span class="sub">{g.share.toFixed(0)}%</span>
									</div>
									<div class="share" aria-hidden="true"><span style="width: {g.share}%"></span></div>
								</li>
							{/each}
						</ul>
					</section>
				{/if}
			</div>
		{/if}
	{/if}
</div>

<style>
	.value-page {
		padding-bottom: 40px;
	}

	.back {
		color: var(--text2);
		text-decoration: none;
		font-size: 0.85rem;
	}

	.back:hover { color: var(--accent-text); }

	.page-header:first-child { padding-bottom: 0; }

	.title-row {
		align-items: center;
		flex-wrap: wrap;
	}

	.range-seg { margin-left: auto; }

	.kpis {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
		gap: 16px;
		padding: 8px 20px 16px;
	}

	.kpi-value {
		font-family: 'Cinzel', serif;
		font-size: 1.6rem;
		font-weight: 600;
		color: var(--accent-text);
		line-height: 1.25;
		font-variant-numeric: tabular-nums;
		overflow-wrap: anywhere;
	}

	.kpi-value.pos, .strong.pos { color: var(--success); }
	.kpi-value.neg, .strong.neg { color: var(--danger); }
	.kpi-value.muted { color: var(--text3); }

	.panel {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		padding: 16px 18px;
		margin: 0 20px 16px;
		display: flex;
		flex-direction: column;
		gap: 12px;
		min-width: 0;
	}

	.lists {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(min(420px, 100%), 1fr));
		gap: 16px;
		padding: 0 20px 16px;
	}

	.lists .panel { margin: 0; }

	.panel-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: 12px;
		flex-wrap: wrap;
	}

	h2 {
		margin: 0;
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text2);
	}

	.panel-sub {
		margin-top: 3px;
		font-size: 0.8rem;
		color: var(--text2);
	}

	.panel-controls {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}

	.strong { color: var(--text); font-weight: 600; }
	.muted { color: var(--text3); }

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
		font-size: 0.78rem;
		padding: 4px 11px;
		cursor: pointer;
	}

	.seg button + button { border-left: 1px solid var(--border2); }

	.seg button.active {
		background: var(--surface2);
		color: var(--text);
		font-weight: 600;
	}

	.note, .list-empty {
		margin: 0;
		font-size: 0.78rem;
		color: var(--text3);
	}

	.list-empty { padding: 12px 0; }

	.rank {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
	}

	.rank li {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 2px 12px;
		padding: 7px 0;
		border-bottom: 1px solid var(--border);
	}

	.rank li:last-child { border-bottom: none; }

	.card-cell {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.card-name {
		font-weight: 600;
		font-size: 0.86rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.card-meta {
		font-size: 0.74rem;
		color: var(--text2);
		font-family: 'JetBrains Mono', monospace;
	}

	.num {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		font-size: 0.86rem;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.num .sub {
		font-size: 0.74rem;
		color: var(--text2);
	}

	.share {
		grid-column: 1 / -1;
		height: 3px;
		background: var(--border);
		border-radius: 2px;
		overflow: hidden;
	}

	.share span {
		display: block;
		height: 100%;
		background: var(--chart-1, var(--accent));
		border-radius: 2px;
	}

	@media (max-width: 640px) {
		.kpis, .lists { padding-left: 16px; padding-right: 16px; }
		.panel { margin-left: 16px; margin-right: 16px; }
		.range-seg { margin-left: 0; }
	}
</style>
