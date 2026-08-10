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

// Generic TTL-keyed cache — backs both the long-lived general cache (system
// info, collections, sets) and the short-lived stats cache (count + value).
function ttlCache(ttlMs: number) {
	const store: Map<string, { data: unknown; ts: number }> = new Map();
	return {
		get<T>(key: string, fetcher: () => Promise<T>): Promise<T> {
			const entry = store.get(key);
			if (entry && Date.now() - entry.ts < ttlMs) return Promise.resolve(entry.data as T);
			return fetcher().then(data => { store.set(key, { data, ts: Date.now() }); return data; });
		},
		delete(key: string) { store.delete(key); },
		clear() { store.clear(); },
		keys() { return store.keys(); }
	};
}

// General TTL cache (system info, collections, sets)
const cache = ttlCache(5 * 60 * 1000);

// Long-lived card detail cache (cards don't change)
const cardDetailCache: Map<string, MtgCard | RiftboundCard | PokemonCard> = new Map();

// Per-card price cache keyed by `provider:id` — prices change rarely within a session
const priceCache: Map<string, CardPrices> = new Map();

// Purchase history cache keyed by `collection:cardId` — invalidated on mutation
const purchaseHistoryCache: Map<string, PurchaseEntry[]> = new Map();

// Collection stats cache (count + value) — short TTL, invalidated on mutation
const statsCache = ttlCache(60 * 1000);
const cachedStats = statsCache.get;

export function invalidateCollectionStats(collection?: string) {
	if (!collection) { statsCache.clear(); return; }
	for (const k of statsCache.keys()) {
		if (k.startsWith(`stats:${collection}`)) statsCache.delete(k);
	}
	purchaseHistoryCache.forEach((_, k) => {
		if (k.startsWith(`${collection}:`)) purchaseHistoryCache.delete(k);
	});
}

function invalidatePurchaseHistory(collection: string) {
	for (const k of purchaseHistoryCache.keys()) {
		if (k.startsWith(`${collection}:`)) purchaseHistoryCache.delete(k);
	}
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

const cachedFetch = cache.get;

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
	invalidateCache('system');
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

export async function renameCollection(oldId: string, newId: string): Promise<void> {
	await fetchJSON(`/api/collection/rename/${encodeURIComponent(oldId)}`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ new_id: newId })
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
	if (filters.provider) params.set('provider', filters.provider);
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
			wantQuantity: entry.wantQuantity ?? 0,
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

// Shareable, read-only collection view — a single request returns a page of
// fully merged card data (entry + card details), unlike getCollectionCards
// which needs a follow-up batch lookup per provider. Lives under /api/share
// so a reverse proxy can expose just this prefix (plus the /share webui2
// route) without authenticating the rest of the app. Reachable only via an
// opaque share token the owner explicitly created — the collection name
// alone grants no access.
export interface PublicCollectionPage {
	cards: CollectionCard[];
	total: number;
}

export async function getPublicCollectionCards(
	token: string,
	page: number,
	sortBy = '',
	sortOrder = 'Asc'
): Promise<PublicCollectionPage> {
	const params = new URLSearchParams({
		offset: String((page - 1) * PAGE_SIZE),
		limit: String(PAGE_SIZE)
	});
	if (sortBy) params.set('sort_by', sortBy);
	if (sortOrder !== 'Asc') params.set('sort_order', sortOrder);
	return fetchJSON(`/api/share/${encodeURIComponent(token)}?${params}`);
}

// Share link management — owner-only, lives under /api/collection so it's
// never exposed by a proxy exception scoped to /api/share.
export interface ShareLink {
	token: string;
	collectionId: string;
	createdAt: string;
}

export async function listShareLinks(collection: string): Promise<ShareLink[]> {
	return fetchJSON(`/api/collection/share/${encodeURIComponent(collection)}`);
}

export async function createShareLink(collection: string): Promise<ShareLink> {
	return fetchJSON(`/api/collection/share/${encodeURIComponent(collection)}`, {
		method: 'POST'
	});
}

export async function revokeShareLink(collection: string, token: string): Promise<void> {
	await fetchJSON(`/api/collection/share/${encodeURIComponent(collection)}/${encodeURIComponent(token)}`, {
		method: 'DELETE'
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
	const params = new URLSearchParams();
	if (filters.provider) params.set('provider', filters.provider);
	return fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/search/count?${params}`, {
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

export async function adjustWantQuantity(collection: string, cardId: string, delta: number): Promise<void> {
	await fetchJSON(`/api/collection/cards/${encodeURIComponent(collection)}/want`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ id: cardId, delta })
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

export async function getRandomMtgCard(): Promise<MtgCard> {
	return fetchJSON('/api/mtg/cards/random');
}

export async function getRandomRiftboundCard(): Promise<RiftboundCard> {
	return fetchJSON('/api/riftbound/cards/random');
}

export async function getRandomPokemonCard(): Promise<PokemonCard> {
	return fetchJSON('/api/pokemon/cards/random');
}

export async function getMtgCardSets(): Promise<CardSet[]> {
	return cachedFetch('mtg-sets', () => fetchJSON<CardSet[]>('/api/mtg/sets'));
}

export async function getPokemonCardSets(): Promise<CardSet[]> {
	return cachedFetch('pokemon-sets', () => fetchJSON<CardSet[]>('/api/pokemon/sets'));
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
	invalidatePurchaseHistory(collection);
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
	invalidatePurchaseHistory(collection);
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
	const pokedex = parseInt(filters.pokedex, 10);
	if (!isNaN(pokedex)) body.pokedex = pokedex;
	if (filters.sortBy) body.sortBy = filters.sortBy;
	if (filters.sortOrder) body.sortOrder = filters.sortOrder;

	// MTG-only advanced filters
	const manaMin = parseFloat(filters.manaValueMin);
	if (!isNaN(manaMin)) body.manaValueMin = manaMin;
	const manaMax = parseFloat(filters.manaValueMax);
	if (!isNaN(manaMax)) body.manaValueMax = manaMax;
	if (filters.colors.length) body.colors = filters.colors;
	const keywords = filters.keywords.split(',').map(k => k.trim()).filter(Boolean);
	if (keywords.length) body.keywords = keywords;
	if (filters.power) body.power = filters.power;
	if (filters.toughness) body.toughness = filters.toughness;
	if (filters.loyalty) body.loyalty = filters.loyalty;
	if (filters.defense) body.defense = filters.defense;
	if (filters.isReserved) body.isReserved = filters.isReserved === 'true';
	if (filters.isPromo) body.isPromo = filters.isPromo === 'true';
	if (filters.isReprint) body.isReprint = filters.isReprint === 'true';
	if (filters.isFullArt) body.isFullArt = filters.isFullArt === 'true';
	if (filters.borderColor) body.borderColor = filters.borderColor;
	if (filters.legalIn) body.legalIn = filters.legalIn;
	return body;
}

export { PAGE_SIZE };
