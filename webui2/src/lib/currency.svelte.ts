import type { CardPrices } from '$lib/types';
import { app } from '$lib/state.svelte';

export const DEFAULT_CURRENCY = 'USD';
// Matches the server's `preferred_currency` default.
const DEFAULT_PREFERRED_CURRENCY = 'EUR';
// Currencies offered in settings; retailers only list in these today.
export const PREFERRED_CURRENCY_OPTIONS = ['EUR', 'USD'];

// ECB reference rates, no API key, CORS-enabled. Only fetched when the
// viewer opts in to converted totals on the settings page.
const RATES_URL = 'https://api.frankfurter.dev/v1/latest?base=EUR';
const COOKIE_NAME = 'gathers-convert-total';

/** The server-configured currency prices are preferably taken in. */
export function preferredCurrency(): string {
	return app.systemInfo?.preferred_currency ?? DEFAULT_PREFERRED_CURRENCY;
}

export interface Money {
	currency: string;
	value: number;
}

// Cache formatters; Intl.NumberFormat construction isn't free and price
// formatting runs once per rendered card.
const formatters = new Map<string, Intl.NumberFormat>();

export function formatMoney(value: number, currency: string = DEFAULT_CURRENCY): string {
	let fmt = formatters.get(currency);
	if (!fmt) {
		try {
			// Fixed 'en' locale: price strings get parsed back into numbers
			// (see QtyControls), which a "1.234,56 €" style would break.
			fmt = new Intl.NumberFormat('en', { style: 'currency', currency, minimumFractionDigits: 2, maximumFractionDigits: 2 });
		} catch {
			// Unknown/invalid currency code: fall back to "12.34 XYZ".
			return `${value.toFixed(2)} ${currency}`;
		}
		formatters.set(currency, fmt);
	}
	return fmt.format(value);
}

const cheapest = (r: { normal?: number | null; foil?: number | null }) =>
	Math.min(r.normal ?? Infinity, r.foil ?? Infinity);

/** Display symbol for a currency code, e.g. "EUR" → "€" (the code itself
 *  if the browser doesn't know it). */
export function currencySymbol(currency: string = preferredCurrency()): string {
	try {
		return new Intl.NumberFormat('en', { style: 'currency', currency })
			.formatToParts(0)
			.find(p => p.type === 'currency')?.value ?? currency;
	} catch {
		return currency;
	}
}

// Same choice the server makes for collection totals (see
// `preferred_retailer`): "raw", else — among retailers listing in the
// preferred currency, or all of them when none do — cardmarket, else the
// cheapest (ties broken by name).
function preferredRetailer(cardPrices: CardPrices) {
	const name = preferredRetailerName(cardPrices);
	return name ? cardPrices.paper[name] : undefined;
}

/** Key (in `CardPrices.paper`) of the retailer a card is valued at. */
export function preferredRetailerName(cardPrices: CardPrices): string | undefined {
	const paper = cardPrices.paper;
	if (paper['raw']) return 'raw';
	const listed = Object.entries(paper).filter(([, r]) => r.normal != null || r.foil != null);
	const preferred = preferredCurrency();
	const inPreferred = listed.filter(([, r]) => (r.currency ?? DEFAULT_CURRENCY) === preferred);
	const candidates = inPreferred.length ? inPreferred : listed;
	return candidates.find(([k]) => k.toLowerCase() === 'cardmarket')?.[0]
		?? candidates.sort(([ka, a], [kb, b]) => cheapest(a) - cheapest(b) || (ka < kb ? -1 : ka > kb ? 1 : 0))[0]?.[0];
}

// Unit price of one finish at the preferred retailer — same rule the server
// uses for totals: "foil" uses the foil price, every other finish the normal
// price, each falling back to the other when missing.
export function finishPrice(cardPrices: CardPrices | undefined, finish = ''): Money | null {
	if (!cardPrices?.paper) return null;
	const preferred = preferredRetailer(cardPrices);
	if (!preferred) return null;
	const value = finish === 'foil'
		? preferred.foil ?? preferred.normal
		: preferred.normal ?? preferred.foil;
	if (value == null) return null;
	return { value, currency: preferred.currency ?? DEFAULT_CURRENCY };
}

export function formatFinishPrice(cardPrices: CardPrices | undefined, finish = ''): string | null {
	const p = finishPrice(cardPrices, finish);
	return p ? formatMoney(p.value, p.currency) : null;
}

// Price label for a card: one price per finish in `finishes` ("" = normal),
// e.g. "€1.20 / €3.40✦" for a card owned in both normal and foil.
export function priceLabel(cardPrices: CardPrices | undefined, finishes: string[] = ['']): string | null {
	const parts = [...new Set(finishes.length ? finishes : [''])]
		.map(f => {
			const p = formatFinishPrice(cardPrices, f);
			return p && (f ? `${p}✦` : p);
		})
		.filter((p): p is string => !!p);
	return parts.length ? parts.join(' / ') : null;
}

// Add `items` into per-currency sums, keeping the input order of first
// appearance.
export function sumByCurrency(items: Money[]): Money[] {
	const sums = new Map<string, number>();
	for (const { currency, value } of items) {
		sums.set(currency, (sums.get(currency) ?? 0) + value);
	}
	return [...sums].map(([currency, value]) => ({ currency, value }));
}

// Preferred currency first, e.g. "€12.00 + $3.40".
export function formatMoneyList(items: Money[]): string {
	const preferred = preferredCurrency();
	return items
		.filter(m => m.value !== 0)
		.sort((a, b) => Number(b.currency === preferred) - Number(a.currency === preferred))
		.map(m => formatMoney(m.value, m.currency))
		.join(' + ');
}

// --- Converted-total preference (cookie) ---

function readCookie(): boolean {
	if (typeof document === 'undefined') return false;
	return document.cookie.split('; ').some(c => c === `${COOKIE_NAME}=1`);
}

export const currencyPrefs = $state({ convertTotals: readCookie() });

/** Whether converted totals are shown. Always on in demo mode, where the
 *  settings page (and so the toggle) isn't available. */
export function convertTotalsEnabled(): boolean {
	return currencyPrefs.convertTotals || (app.systemInfo?.demo_mode ?? false);
}

export function setConvertTotals(enabled: boolean) {
	currencyPrefs.convertTotals = enabled;
	if (typeof document !== 'undefined') {
		const maxAge = enabled ? 60 * 60 * 24 * 365 * 5 : 0;
		document.cookie = `${COOKIE_NAME}=${enabled ? 1 : 0}; path=/; max-age=${maxAge}; SameSite=Lax`;
	}
}

// --- Exchange rates ---

// 1 EUR = rates[code] units of `code`.
let ratesPromise: Promise<Record<string, number> | null> | null = null;

export function getRates(): Promise<Record<string, number> | null> {
	if (!ratesPromise) {
		ratesPromise = fetch(RATES_URL)
			.then(r => (r.ok ? r.json() : null))
			.then(j => (j?.rates ? { ...j.rates, EUR: 1 } : null))
			.catch(() => null);
		// Retry on a later call if this one failed.
		ratesPromise.then(r => { if (!r) ratesPromise = null; });
	}
	return ratesPromise;
}

// Sum everything into `target`. Returns null if any currency has no known
// rate, rather than a silently incomplete total.
export function convertTotal(items: Money[], target: string, rates: Record<string, number>): number | null {
	const targetRate = rates[target];
	if (!targetRate) return null;
	let total = 0;
	for (const { currency, value } of items) {
		const rate = rates[currency];
		if (!rate) return null;
		total += (value / rate) * targetRate;
	}
	return total;
}
