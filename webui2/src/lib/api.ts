import type {
	Collection,
	SystemInfo,
	CollectionEntry,
	CollectionCard,
	MtgCard,
	RiftboundCard,
	PokemonCard,
	CardSet,
	SearchFilters,
	CardPrices,
	ValueBreakdown,
	Settings
} from './types';

const PAGE_SIZE = 24;

// General TTL cache (system info, collections, sets)
const cache: Map<string, { data: unknown; ts: number }> = new Map();
const CACHE_TTL = 5 * 60 * 1000; // 5 min

// Long-lived card detail cache (cards don't change)
const cardDetailCache: Map<string, MtgCard | RiftboundCard | PokemonCard> = new Map();

// Per-card price cache keyed by `provider:id` — prices change rarely within a session
const priceCache: Map<string, CardPrices> = new Map();

// Purchase history cache keyed by `collection:cardId` — invalidated on mutation
const purchaseHistoryCache: Map<string, PurchaseEntry[]> = new Map();

// Collection stats cache (count + value) — short TTL, invalidated on mutation
const STATS_TTL = 60 * 1000; // 1 min
const statsCache: Map<string, { data: unknown; ts: number }> = new Map();

function cachedStats<T>(key: string, fetcher: () => Promise<T>): Promise<T> {
	const entry = statsCache.get(key);
	if (entry && Date.now() - entry.ts < STATS_TTL) return Promise.resolve(entry.data as T);
	return fetcher().then(data => { statsCache.set(key, { data, ts: Date.now() }); return data; });
}

export function invalidateCollectionStats(collection?: string) {
	if (!collection) { statsCache.clear(); return; }
	for (const k of statsCache.keys()) {
		if (k.startsWith(`stats:${collection}`)) statsCache.delete(k);
	}
	purchaseHistoryCache.forEach((_, k) => {
		if (k.startsWith(`${collection}:`)) purchaseHistoryCache.delete(k);
	});
}

async function fetchJSON<T>(url: string, options?: RequestInit): Promise<T> {
	const res = await fetch(url, options);
	if (!res.ok) {
		// Try to extract a human-readable error message from the response body
		const body = await res.json().catch(() => null) as { error?: string } | null;
		throw new Error(body?.error ?? `${res.status} ${res.statusText}`);
	}
	// 204 No Content — return undefined cast to T
	if (res.status === 204) return undefined as unknown as T;
	return res.json();
}

async function cachedFetch<T>(key: string, fetcher: () => Promise<T>): Promise<T> {
	const entry = cache.get(key);
	if (entry && Date.now() - entry.ts < CACHE_TTL) return entry.data as T;
	const data = await fetcher();
	cache.set(key, { data, ts: Date.now() });
	return data;
}

export function invalidateCache(prefix?: string) {
	if (!prefix) { cache.clear(); return; }
	for (const key of cache.keys()) {
		if (key.startsWith(prefix)) cache.delete(key);
	}
}

// System
export async function getSystemInfo(): Promise<SystemInfo> {
	return cachedFetch('system', () => fetchJSON<SystemInfo>('/api/system'));
}

export function invalidateSystemInfo() {
	cache.delete('system');
}

// Collections
export async function listCollections(): Promise<Collection[]> {
	return cachedFetch('collections', () => fetchJSON<Collection[]>('/api/collection/list'));
}

export async function addCollection(id: string): Promise<void> {
	await fetchJSON('/api/collection/add', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ id })
	});
	invalidateCache('collections');
}

export async function deleteCollection(id: string, keepCardsIn = ''): Promise<void> {
	await fetchJSON(`/api/collection/remove/${encodeURIComponent(id)}?keepCardsInCollection=${encodeURIComponent(keepCardsIn)}`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' }
	});
	invalidateCache('collections');
}

// Collection cards — raw entries (id + quantities only)
async function getRawCollectionEntries(
	collection: string,
	page: number,
	sortBy = '',
	sortOrder = 'Asc',
	provider = ''
): Promise<CollectionEntry[]> {
	const params = new URLSearchParams({
		offset: String((page - 1) * PAGE_SIZE),
		limit: String(PAGE_SIZE)
	});
	if (sortBy) params.set('sort_by', sortBy);
	if (sortOrder !== 'Asc') params.set('sort_order', sortOrder);
	if (provider) params.set('provider', provider);
	return fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/list?${params}`);
}

async function getRawSearchEntries(
	collection: string,
	filters: SearchFilters,
	page: number
): Promise<CollectionEntry[]> {
	const params = new URLSearchParams({
		offset: String((page - 1) * PAGE_SIZE),
		limit: String(PAGE_SIZE)
	});
	return fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/search?${params}`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(buildSearchBody(filters))
	});
}

// Bulk card detail lookup — batches IDs and caches results
async function fetchCardDetails(
	ids: string[],
	provider: string
): Promise<Record<string, MtgCard | RiftboundCard | PokemonCard>> {
	const missing = ids.filter(id => !cardDetailCache.has(`${provider}:${id}`));
	if (missing.length > 0) {
		const endpoint = provider === 'RiftboundSQLite'
			? '/api/riftbound/cards'
			: provider === 'PokemonSQLite'
			? '/api/pokemon/cards'
			: '/api/mtg/cards';
		// Build ?ids=x&ids=y query
		const params = missing.map(id => `ids=${encodeURIComponent(id)}`).join('&');
		try {
			const results = await fetchJSON<Record<string, MtgCard | RiftboundCard | PokemonCard>>(
				`${endpoint}?${params}`
			);
			for (const [id, detail] of Object.entries(results)) {
				cardDetailCache.set(`${provider}:${id}`, detail);
			}
		} catch (err) {
			console.error(`[gathers] fetchCardDetails failed for provider=${provider}:`, err);
		}
	}
	const out: Record<string, MtgCard | RiftboundCard | PokemonCard> = {};
	for (const id of ids) {
		const detail = cardDetailCache.get(`${provider}:${id}`);
		if (detail) out[id] = detail;
	}
	return out;
}

// Enrich entries with card details, batched by provider
async function enrichEntries(entries: CollectionEntry[]): Promise<CollectionCard[]> {
	// Group by provider
	const byProvider: Record<string, string[]> = {};
	for (const entry of entries) {
		const p = entry.provider || 'MagicSQLite';
		if (!byProvider[p]) byProvider[p] = [];
		byProvider[p].push(entry.id);
	}

	// Fetch details for each provider group in parallel
	const detailMaps = await Promise.all(
		Object.entries(byProvider).map(([provider, ids]) =>
			fetchCardDetails(ids, provider).then(details => ({ provider, details }))
		)
	);

	// Merge into a flat map
	const allDetails: Record<string, MtgCard | RiftboundCard | PokemonCard> = {};
	for (const { details } of detailMaps) {
		Object.assign(allDetails, details);
	}

	// Merge entry + details
	return entries.map(entry => {
		const detail = allDetails[entry.id] ?? {};
		return {
			...detail,
			id: entry.id,
			quantity: entry.quantity,
			foilQuantity: entry.foilQuantity,
			collectionId: entry.collectionId,
			timeAdded: entry.timeAdded,
			provider: entry.provider || 'MagicSQLite',
		} as CollectionCard;
	});
}

export async function getCollectionCards(
	collection: string,
	page: number,
	sortBy = '',
	sortOrder = 'Asc',
	provider = ''
): Promise<CollectionCard[]> {
	const entries = await getRawCollectionEntries(collection, page, sortBy, sortOrder, provider);
	return enrichEntries(entries);
}

export async function getCollectionCount(collection: string, provider = ''): Promise<number> {
	const key = `stats:${collection}:count:${provider}`;
	return cachedStats(key, () => {
		const params = new URLSearchParams();
		if (provider) params.set('provider', provider);
		return fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/count?${params}`);
	});
}

export async function searchCollectionCards(
	collection: string,
	filters: SearchFilters,
	page: number
): Promise<CollectionCard[]> {
	const entries = await getRawSearchEntries(collection, filters, page);
	return enrichEntries(entries);
}

export async function searchCollectionCount(
	collection: string,
	filters: SearchFilters
): Promise<number> {
	return fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/search/count`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(buildSearchBody(filters))
	});
}

export async function addCardToCollection(
	collection: string,
	cardId: string,
	quantity = 1,
	foilQuantity = 0,
	purchasePrice?: number | null
): Promise<void> {
	await fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/add`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({
			id: cardId,
			quantity,
			foilQuantity,
			...(purchasePrice != null ? { purchasePrice } : {})
		})
	});
	invalidateCollectionStats(collection);
}

export async function deleteCardFromCollection(collection: string, cardId: string, quantity: number, foilQuantity: number): Promise<void> {
	await fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/delete`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ id: cardId, quantity, foilQuantity })
	});
	invalidateCollectionStats(collection);
}

export async function moveCards(
	fromCollection: string,
	toCollection: string,
	cards: Array<{ id: string; quantity: number; foilQuantity: number }>
): Promise<void> {
	await fetchJSON(`/api/collection/move/${encodeURIComponent(toCollection)}`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(cards)
	});
}

export async function importCards(collection: string, file: File): Promise<void> {
	const form = new FormData();
	form.append('file', file);
	form.append('collection', collection);
	await fetch('/api/collection/import', { method: 'POST', body: form });
}

export function exportCollectionUrl(collection: string): string {
	return `/api/collection/export/${encodeURIComponent(collection)}`;
}

export async function getCollectionValue(collection: string): Promise<ValueBreakdown> {
	return cachedStats(`stats:${collection}:value`, () =>
		fetchJSON<ValueBreakdown>(`/api/collection/cards/${encodeURIComponent(collection)}/value_breakdown`).catch(() => ({} as ValueBreakdown))
	);
}

// MTG search
export async function searchMtg(filters: SearchFilters, page: number): Promise<MtgCard[]> {
	return fetchJSON(`/api/mtg/cards/search?limit=${PAGE_SIZE}&skip=${(page - 1) * PAGE_SIZE}`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(buildSearchBody(filters))
	});
}

export async function searchRiftbound(filters: SearchFilters, page: number): Promise<RiftboundCard[]> {
	return fetchJSON(`/api/riftbound/cards/search?limit=${PAGE_SIZE}&skip=${(page - 1) * PAGE_SIZE}`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(buildSearchBody(filters))
	});
}

export async function searchPokemon(filters: SearchFilters, page: number): Promise<PokemonCard[]> {
	return fetchJSON(`/api/pokemon/cards/search?limit=${PAGE_SIZE}&skip=${(page - 1) * PAGE_SIZE}`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(buildSearchBody(filters))
	});
}

export async function getMtgCardSets(): Promise<CardSet[]> {
	return cachedFetch('mtg-sets', () => fetchJSON<CardSet[]>('/api/mtg/sets'));
}

async function fetchPrices(endpoint: string, ids: string[]): Promise<Record<string, CardPrices>> {
	const missing = ids.filter(id => !priceCache.has(id));
	if (missing.length) {
		const params = missing.map(id => `ids=${encodeURIComponent(id)}`).join('&');
		const result = await fetchJSON<Record<string, CardPrices>>(`${endpoint}?${params}`).catch(() => ({} as Record<string, CardPrices>));
		for (const [id, prices] of Object.entries(result)) priceCache.set(id, prices);
	}
	return Object.fromEntries(ids.flatMap(id => priceCache.has(id) ? [[id, priceCache.get(id)!]] : []));
}

export async function getMtgPrices(ids: string[]): Promise<Record<string, CardPrices>> {
	if (!ids.length) return {};
	return fetchPrices('/api/mtg/prices', ids);
}

export async function getPokemonPrices(ids: string[]): Promise<Record<string, CardPrices>> {
	if (!ids.length) return {};
	return fetchPrices('/api/pokemon/prices', ids);
}

// Purchase history
export interface PurchaseEntry {
	id: number;
	card_uuid: string;
	card_name: string;
	set_code: string;
	quantity: number;
	foil_quantity: number;
	normal_price_per_unit: number | null;
	foil_price_per_unit: number | null;
	provider: string;
	recorded_at: string;
}

export async function getAllPurchaseHistory(collection: string): Promise<PurchaseEntry[]> {
	const data = await fetchJSON<{ entries: PurchaseEntry[] }>(
		`/api/collection/cards/${encodeURIComponent(collection)}/purchase_history`
	).catch(() => ({ entries: [] }));
	return data.entries;
}

export async function deletePurchaseEntry(collection: string, entryId: number): Promise<void> {
	await fetch(
		`/api/collection/cards/${encodeURIComponent(collection)}/purchase_history_entry/${entryId}`,
		{ method: 'DELETE' }
	);
	// Invalidate per-card cache entries for this collection
	for (const k of purchaseHistoryCache.keys()) {
		if (k.startsWith(`${collection}:`)) purchaseHistoryCache.delete(k);
	}
}

export async function updatePurchaseEntry(
	collection: string,
	entryId: number,
	quantity: number,
	foil_quantity: number,
	normal_price_per_unit: number | null,
	foil_price_per_unit: number | null
): Promise<void> {
	await fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/purchase_history_entry/${entryId}`, {
		method: 'PATCH',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ quantity, foil_quantity, normal_price_per_unit, foil_price_per_unit })
	});
	for (const k of purchaseHistoryCache.keys()) {
		if (k.startsWith(`${collection}:`)) purchaseHistoryCache.delete(k);
	}
}

export async function getPurchaseHistory(collection: string, cardId: string): Promise<PurchaseEntry[]> {
	const key = `${collection}:${cardId}`;
	if (purchaseHistoryCache.has(key)) return purchaseHistoryCache.get(key)!;
	const data = await fetchJSON<{ entries: PurchaseEntry[] }>(
		`/api/collection/cards/${encodeURIComponent(collection)}/purchase_history/${encodeURIComponent(cardId)}`
	).catch(() => ({ entries: [] }));
	purchaseHistoryCache.set(key, data.entries);
	return data.entries;
}

// Settings
export async function getSettings(): Promise<Settings> {
	return fetchJSON('/api/settings');
}

export async function saveSettings(settings: Settings): Promise<Settings> {
	return fetchJSON('/api/settings', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(settings)
	});
}

export async function triggerUpdate(endpoint: string): Promise<string> {
	const result = await fetchJSON<string | boolean>(endpoint);
	return typeof result === 'string' ? result : 'Done';
}

function buildSearchBody(filters: SearchFilters): Record<string, unknown> {
	const body: Record<string, unknown> = {};
	if (filters.name) body.name = filters.name;
	if (filters.setCode) body.setCode = filters.setCode;
	if (filters.rarity) body.rarity = filters.rarity;
	if (filters.artist) body.artist = filters.artist;
	if (filters.text) body.text = filters.text;
	if (filters.collectorNumber) body.collectorNumber = filters.collectorNumber;
	if (filters.colorIdentities.length) body.colorIdentities = filters.colorIdentities;
	if (filters.domains.length) body.domains = filters.domains;
	if (filters.energyTypes.length) body.energyTypes = filters.energyTypes;
	if (filters.sortBy) body.sortBy = filters.sortBy;
	if (filters.sortOrder) body.sortOrder = filters.sortOrder;
	return body;
}

export { PAGE_SIZE };
