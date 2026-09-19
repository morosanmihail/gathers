import type { components } from './generated/api';

/** Same as T, but the given keys become optional instead of required. */
export type PartialBy<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>;

export type Theme = string;
export type ViewMode = 'grid' | 'list';
export type Provider = 'MagicSQLite' | 'RiftboundSQLite' | 'PokemonSQLite' | 'Scryfall';

export interface Collection {
	id: string;
}

export type SystemInfo = components['schemas']['SystemInfo'];
export type DownloadProgress = components['schemas']['DownloadProgressInfo'];
export type CardIdentifiers = components['schemas']['APICardIdentifiers'];
export type MtgCard = components['schemas']['APICard'];
export type RiftboundCard = components['schemas']['APIRiftboundCard'];
export type PokemonCard = components['schemas']['APIPokemonCard'];
export type PluginCardWire = components['schemas']['PluginCard'];

/** Search-result shape for a third-party plugin card (see retrieval::systems::plugin
 *  and the `dummy-plugin` example crate). Not addable to a collection — collections
 *  are built around the closed Magic/Riftbound/Pokemon `Card` enum on the server,
 *  so a plugin-sourced card has nowhere to be stored yet. */
export type PluginResultCard = {
	id: string;
	name: string;
	setCode?: string;
	setName?: string;
	collectorNumber?: string;
	description?: string;
	image?: string;
	rarity?: string;
	/** Name of the plugin this card came from — distinguishes it from a real system. */
	provider: string;
};

export type AnyCard = MtgCard | RiftboundCard | PokemonCard | PluginResultCard;

// Raw response from /api/collection/cards/{id}/list — no card details.
// One row per (uuid, finish): a card owned in two finishes (e.g. nonfoil +
// foil) is two entries sharing the same `id`. See `CardGroup`/`groupByCard`
// below for how the UI puts those back together as "one card, several
// finishes".
export type CollectionEntry = components['schemas']['CollectionCard'];

// CollectionEntry merged with card detail fields
export interface CollectionCard extends PartialBy<CollectionEntry, 'timeAdded' | 'provider' | 'wantQuantity' | 'finish'> {
	image?: string;
	name: string;
	setCode?: string;
	collectorNumber?: string;
	rarity?: string;
	artist?: string;
	text?: string;
	colorIdentity?: string[];
	cardIdentifiers?: CardIdentifiers;
	types?: string[];
	supertypes?: string[];
	subtypes?: string[];
	domain?: string;
	imageUrl?: string;
	energyTypes?: string[];
	mtGCard?: MtgCard;
	// The finishes this card is actually printed in — mtgjson's for MTG
	// (e.g. ["nonfoil", "foil"], sometimes "etched") or the scraped
	// `variants` for Pokemon (e.g. ["Normal", "Reverse Holofoil"]). Drives
	// which finishes the "add another version" picker offers. Absent for
	// games with no such catalog data yet (Riftbound).
	finishes?: string[];
}

// One card (by uuid), grouped back together from its individual per-finish
// `CollectionCard` rows. `entries` holds every finish actually present
// (quantity or want > 0 on the "" row); the group's own top-level fields
// (name, image, quantity, finish, ...) mirror one representative entry —
// prefer the "" (default) finish since that's where `wantQuantity` lives.
export interface CardGroup extends CollectionCard {
	entries: CollectionCard[];
}

export function groupByCard(cards: CollectionCard[]): CardGroup[] {
	const order: string[] = [];
	const byId = new Map<string, CollectionCard[]>();
	for (const c of cards) {
		if (!byId.has(c.id)) { byId.set(c.id, []); order.push(c.id); }
		byId.get(c.id)!.push(c);
	}
	return order.map(id => {
		const entries = byId.get(id)!;
		const primary = entries.find(e => !e.finish) ?? entries[0];
		return { ...primary, entries };
	});
}

// Human label for a finish value — "" (the default/primary finish) reads
// as "Normal", anything else is title-cased as-is (MTG's "foil"/"etched",
// a Pokemon variant like "reverse holo", ...).
export function finishLabel(finish: string): string {
	if (!finish) return 'Normal';
	return finish.charAt(0).toUpperCase() + finish.slice(1);
}

// Maps a catalog finish value (mtgjson's "nonfoil", Pokemon's "Normal", or
// another game's own finish name) to gathers' internal convention, where ""
// means the default/primary finish.
export function toInternalFinish(catalogFinish: string): string {
	const lower = catalogFinish.toLowerCase();
	return lower === 'nonfoil' || lower === 'normal' ? '' : catalogFinish;
}

// A card's own catalog finishes (MTG, Pokemon — see `CollectionCard.finishes`),
// mapped to gathers' internal finish values. Empty when the game has no such
// catalog data yet (Riftbound).
export function catalogFinishes(card: { finishes?: string[] }): string[] {
	return (card.finishes ?? []).map(toInternalFinish);
}

// Finishes that could still be added to this card group — from its own
// `finishes` catalog data when available, otherwise falling back to a
// generic Normal/Foil choice (the common case for games without per-card
// finish data yet — see `finishes` field doc above). Already-owned finishes
// (quantity > 0) are excluded.
export function availableFinishesToAdd(group: CardGroup): string[] {
	const owned = new Set(group.entries.filter(e => (e.quantity ?? 0) > 0).map(e => e.finish ?? ''));
	const catalog = group.finishes?.length ? catalogFinishes(group) : ['', 'foil'];
	const seen = new Set<string>();
	return catalog.filter(f => {
		if (owned.has(f) || seen.has(f)) return false;
		seen.add(f);
		return true;
	});
}

export type CardSet = components['schemas']['Set'];

export interface SearchFilters {
	// Collection views only: restrict to a single card game's provider (e.g.
	// 'MagicSQLite', 'RiftboundSQLite', 'PokemonSQLite'). '' = all games.
	provider: string;
	name: string;
	setCode: string;
	artist: string;
	text: string;
	rarity: string;
	collectorNumber: string;
	colorIdentities: string[];
	domains: string[];
	energyTypes: string[];
	// Pokemon-only: exact National Pokédex number match.
	pokedex: string;
	sortBy: string;
	sortOrder: 'Asc' | 'Desc';
	// MTG-only advanced filters
	manaValueMin: string;
	manaValueMax: string;
	colors: string[];
	keywords: string;
	power: string;
	toughness: string;
	loyalty: string;
	defense: string;
	isReserved: TriState;
	isPromo: TriState;
	isReprint: TriState;
	isFullArt: TriState;
	borderColor: string;
	legalIn: string;
}

// '' = don't filter, 'true'/'false' = require present/absent
export type TriState = '' | 'true' | 'false';

export type CardPrices = components['schemas']['CardPrices'];

// True when a card group is tracked purely as a wishlist entry — none of
// its finishes owned yet, only a desired quantity.
export function isWantOnly(group: CardGroup): boolean {
	return group.entries.every(e => (e.quantity ?? 0) === 0) && (group.wantQuantity ?? 0) > 0;
}

export function rarityClass(r?: string): string {
	if (!r) return '';
	return `rarity rarity-${r[0].toUpperCase()}`;
}

// Shared filter chip option lists (color identity, Riftbound domains, Pokemon energy types)
export const colorOptions = [
	{ value: 'White', label: 'W' },
	{ value: 'Blue', label: 'U' },
	{ value: 'Black', label: 'B' },
	{ value: 'Red', label: 'R' },
	{ value: 'Green', label: 'G' }
];

// Exact enum values from APICardDomain
export const riftboundDomains = ['Calm', 'Chaos', 'Fury', 'Mind', 'Body', 'Order', 'Colorless'];

// Exact enum values from APIEnergyType (skip 'Energy' — not useful for filtering)
export const pokemonEnergyTypes = [
	'Fire', 'Water', 'Grass', 'Lightning', 'Psychic',
	'Fighting', 'Darkness', 'Metal', 'Dragon', 'Fairy', 'Colorless'
];

// Toggle `value` in/out of a string list, returning a new array
export function toggleInList(list: string[], value: string): string[] {
	return list.includes(value) ? list.filter(v => v !== value) : [...list, value];
}

export function bestPrice(cardPrices: CardPrices): string | null {
	if (!cardPrices?.paper) return null;
	const vals = Object.values(cardPrices.paper).flatMap(r => [r.normal, r.foil].filter(v => v != null)) as number[];
	if (!vals.length) return null;
	return `$${Math.min(...vals).toFixed(2)}`;
}

export type ValueBreakdown = components['schemas']['CollectionValueBreakdown'];
export type Settings = components['schemas']['ServerConfig'];
export type PluginConfig = components['schemas']['PluginConfig'];
export type System = components['schemas']['Systems'];

export function defaultFilters(): SearchFilters {
	return {
		provider: '',
		name: '',
		setCode: '',
		artist: '',
		text: '',
		rarity: '',
		collectorNumber: '',
		colorIdentities: [],
		domains: [],
		energyTypes: [],
		pokedex: '',
		sortBy: 'Name',
		sortOrder: 'Asc',
		manaValueMin: '',
		manaValueMax: '',
		colors: [],
		keywords: '',
		power: '',
		toughness: '',
		loyalty: '',
		defense: '',
		isReserved: '',
		isPromo: '',
		isReprint: '',
		isFullArt: '',
		borderColor: '',
		legalIn: ''
	};
}

// mtgjson `cardLegalities` format columns — must match retrieval::LEGALITY_FORMATS server-side.
export const legalityFormats: { value: string; label: string }[] = [
	{ value: 'standard', label: 'Standard' },
	{ value: 'pioneer', label: 'Pioneer' },
	{ value: 'modern', label: 'Modern' },
	{ value: 'legacy', label: 'Legacy' },
	{ value: 'vintage', label: 'Vintage' },
	{ value: 'commander', label: 'Commander' },
	{ value: 'paupercommander', label: 'Pauper Commander' },
	{ value: 'pauper', label: 'Pauper' },
	{ value: 'brawl', label: 'Brawl' },
	{ value: 'standardbrawl', label: 'Standard Brawl' },
	{ value: 'alchemy', label: 'Alchemy' },
	{ value: 'historic', label: 'Historic' },
	{ value: 'timeless', label: 'Timeless' },
	{ value: 'gladiator', label: 'Gladiator' },
	{ value: 'penny', label: 'Penny Dreadful' },
	{ value: 'duel', label: 'Duel Commander' },
	{ value: 'oathbreaker', label: 'Oathbreaker' },
	{ value: 'predh', label: 'PreDH' },
	{ value: 'premodern', label: 'Premodern' },
	{ value: 'oldschool', label: 'Old School' },
	{ value: 'future', label: 'Future' },
	{ value: 'tlr', label: 'The List' }
];

// Known mtgjson `borderColor` values.
export const borderColors = ['black', 'white', 'borderless', 'silver', 'gold', 'yellow'];

export function cardImageUrl(card: CollectionCard | MtgCard | RiftboundCard | PokemonCard | PluginResultCard): string {
	// Riftbound, Pokemon, and plugin cards store image URL directly
	const directImage = (card as CollectionCard | RiftboundCard | PokemonCard | PluginResultCard).image;
	if (directImage) return directImage;
	// MTG cards use Scryfall identifiers
	const ids = (card as CollectionCard).cardIdentifiers ?? (card as MtgCard).cardIdentifiers;
	if (ids?.scryfallId) {
		return `https://api.scryfall.com/cards/${ids.scryfallId}?format=image`;
	}
	return '';
}
