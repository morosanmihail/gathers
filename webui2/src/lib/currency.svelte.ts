import type { CardPrices } from '$lib/types';

export const DEFAULT_CURRENCY = 'USD';
export const BASE_CURRENCY = 'EUR';

// ECB reference rates, no API key, CORS-enabled. Only fetched when the
// viewer opts in to EUR conversion on the settings page.
const RATES_URL = 'https://api.frankfurter.dev/v1/latest?base=EUR';
const COOKIE_NAME = 'gathers-convert-eur';

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

// Same choice the server makes for collection totals (see
// `preferred_retailer`): raw, then cardmarket, then the cheapest retailer
// (ties broken by name).
function preferredRetailer(cardPrices: CardPrices) {
	const paper = cardPrices.paper;
	return paper['raw']
		?? Object.entries(paper).find(([k]) => k.toLowerCase() === 'cardmarket')?.[1]
		?? Object.entries(paper)
			.filter(([, r]) => r.normal != null || r.foil != null)
			.sort(([ka, a], [kb, b]) => cheapest(a) - cheapest(b) || (ka < kb ? -1 : ka > kb ? 1 : 0))[0]?.[1];
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

export function formatMoneyList(items: Money[]): string {
	return items.filter(m => m.value !== 0).map(m => formatMoney(m.value, m.currency)).join(' + ');
}

// --- EUR conversion preference (cookie) ---

function readCookie(): boolean {
	if (typeof document === 'undefined') return false;
	return document.cookie.split('; ').some(c => c === `${COOKIE_NAME}=1`);
}

export const currencyPrefs = $state({ convertToEur: readCookie() });

export function setConvertToEur(enabled: boolean) {
	currencyPrefs.convertToEur = enabled;
	if (typeof document !== 'undefined') {
		const maxAge = enabled ? 60 * 60 * 24 * 365 * 5 : 0;
		document.cookie = `${COOKIE_NAME}=${enabled ? 1 : 0}; path=/; max-age=${maxAge}; SameSite=Lax`;
	}
}

// --- Exchange rates ---

// 1 EUR = rates[code] units of `code`.
let ratesPromise: Promise<Record<string, number> | null> | null = null;

export function getEurRates(): Promise<Record<string, number> | null> {
	if (!ratesPromise) {
		ratesPromise = fetch(RATES_URL)
			.then(r => (r.ok ? r.json() : null))
			.then(j => (j?.rates ? { ...j.rates, [BASE_CURRENCY]: 1 } : null))
			.catch(() => null);
		// Retry on a later call if this one failed.
		ratesPromise.then(r => { if (!r) ratesPromise = null; });
	}
	return ratesPromise;
}

// Sum everything into EUR. Returns null if any currency has no known rate,
// rather than a silently incomplete total.
export function toEurTotal(items: Money[], rates: Record<string, number>): number | null {
	let total = 0;
	for (const { currency, value } of items) {
		const rate = rates[currency];
		if (!rate) return null;
		total += value / rate;
	}
	return total;
}
