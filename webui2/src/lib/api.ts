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

// General TTL cache
const cache: Map<string, { data: unknown; ts: number }> = new Map();
const CACHE_TTL = 5 * 60 * 1000; // 5 min

// Long-lived card detail cache (cards don't change)
const cardDetailCache: Map<string, MtgCard | RiftboundCard | PokemonCard> = new Map();

async function fetchJSON<T>(url: string, options?: RequestInit): Promise<T> {
	const res = await fetch(url, options);
	if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
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
	const params = new URLSearchParams();
	if (provider) params.set('provider', provider);
	return fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/count?${params}`);
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

export async function addCardToCollection(collection: string, cardId: string, quantity = 1, foilQuantity = 0): Promise<void> {
	await fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/add`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ id: cardId, quantity, foilQuantity })
	});
}

export async function deleteCardFromCollection(collection: string, cardId: string, quantity: number, foilQuantity: number): Promise<void> {
	await fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/delete`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ id: cardId, quantity, foilQuantity })
	});
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
	return fetchJSON<ValueBreakdown>(`/api/collection/cards/${encodeURIComponent(collection)}/value_breakdown`).catch(() => ({} as ValueBreakdown));
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

export async function getMtgPrices(ids: string[]): Promise<Record<string, CardPrices>> {
	if (!ids.length) return {};
	const params = ids.map(id => `ids=${encodeURIComponent(id)}`).join('&');
	return fetchJSON<Record<string, CardPrices>>(`/api/mtg/prices?${params}`).catch(() => ({} as Record<string, CardPrices>));
}

export async function getPokemonPrices(ids: string[]): Promise<Record<string, CardPrices>> {
	if (!ids.length) return {};
	const params = ids.map(id => `ids=${encodeURIComponent(id)}`).join('&');
	return fetchJSON<Record<string, CardPrices>>(`/api/pokemon/prices?${params}`).catch(() => ({} as Record<string, CardPrices>));
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

export async function getPurchaseHistory(collection: string, cardId: string): Promise<PurchaseEntry[]> {
	const data = await fetchJSON<{ entries: PurchaseEntry[] }>(
		`/api/collection/cards/${encodeURIComponent(collection)}/purchase_history/${encodeURIComponent(cardId)}`
	).catch(() => ({ entries: [] }));
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
