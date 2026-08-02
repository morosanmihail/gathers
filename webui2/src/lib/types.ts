export type Theme = string;
export type ViewMode = 'grid' | 'list';
export type Provider = 'MagicSQLite' | 'RiftboundSQLite' | 'PokemonSQLite' | 'Scryfall';

export interface Collection {
	id: string;
}

export interface SystemInfo {
	collections_enabled: boolean;
	systems: string[];
	demo_mode?: boolean;
	downloading?: Record<string, DownloadProgress>;
	pricing_enabled?: boolean;
}

export interface DownloadProgress {
	phase: 'checking' | 'downloading' | 'verifying';
	downloaded: number;
	total: number;
}

export interface CardIdentifiers {
	scryfallId?: string;
	mtgjsonId?: string;
}

export interface MtgCard {
	id: string;
	name: string;
	setCode: string;
	rarity: string;
	artist: string;
	text?: string;
	colorIdentity: string[];
	cardIdentifiers?: CardIdentifiers;
	types: string[];
	supertypes: string[];
	subtypes: string[];
	collectorNumber?: string;
	manaCost?: string;
	power?: string;
	toughness?: string;
	// Extended fields (mtgjson metadata)
	manaValue?: number;
	typeLine?: string;
	loyalty?: string;
	defense?: string;
	keywords?: string[];
	colors?: string[];
	legalities?: Record<string, string>;
	finishes?: string[];
	isReserved?: boolean;
	isPromo?: boolean;
	isReprint?: boolean;
	borderColor?: string;
	frameEffects?: string[];
	isFullArt?: boolean;
	watermark?: string;
	flavorText?: string;
	setName?: string;
}

export interface RiftboundCard {
	id: string;
	name: string;
	setCode?: string;
	collectorNumber?: string;
	rarity?: string;
	artists?: string[];
	domains?: unknown[];
	text?: string;
	image?: string;
}

export interface PokemonCard {
	id: string;
	name: string;
	setCode?: string;
	rarity?: string;
	energyTypes?: unknown[];
	cardType?: string;
	collectorNumber?: string;
	image?: string;
	description?: string;
	releaseDate?: string;
	pokedex?: number;
}

export type AnyCard = MtgCard | RiftboundCard | PokemonCard;

// Raw response from /api/collection/cards/{id}/list — no card details
export interface CollectionEntry {
	id: string;
	quantity: number;
	foilQuantity: number;
	collectionId: string;
	timeAdded: string;
	provider: string;
}

// CollectionEntry merged with card detail fields
export interface CollectionCard {
	collectionId: string;
	id: string;
	quantity: number;
	foilQuantity: number;
	timeAdded?: string;
	provider?: string;
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
}

export interface CardSet {
	code: string;
	name: string;
}

export interface SearchFilters {
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

export interface PriceRetailer {
	normal?: number;
	foil?: number;
}

export interface CardPrices {
	paper?: Record<string, PriceRetailer>;
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

export interface ValueBreakdown {
	total_value?: number;
	profit?: number;
	untracked_value?: number;
	priced_count?: number;
	total_count?: number;
	[key: string]: unknown;
}

export interface Settings {
	system: string[];
	port: number;
	collections_enabled?: boolean;
	pricing_enabled?: boolean;
	mtg_db_path?: string | null;
	mtg_prices_path?: string | null;
	riftbound_db_path?: string | null;
	pokemon_db_path?: string | null;
	pokemon_prices_path?: string | null;
	storage_db_path?: string | null;
	[key: string]: unknown;
}

export function defaultFilters(): SearchFilters {
	return {
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

export function cardImageUrl(card: CollectionCard | MtgCard | RiftboundCard | PokemonCard): string {
	// Riftbound and Pokemon store image URL directly
	const directImage = (card as CollectionCard | RiftboundCard | PokemonCard).image;
	if (directImage) return directImage;
	// MTG cards use Scryfall identifiers
	const ids = (card as CollectionCard).cardIdentifiers ?? (card as MtgCard).cardIdentifiers;
	if (ids?.scryfallId) {
		return `https://api.scryfall.com/cards/${ids.scryfallId}?format=image`;
	}
	return '';
}
