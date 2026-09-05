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

export type AnyCard = MtgCard | RiftboundCard | PokemonCard;

// Raw response from /api/collection/cards/{id}/list — no card details
export type CollectionEntry = components['schemas']['CollectionCard'];

// CollectionEntry merged with card detail fields
export interface CollectionCard extends PartialBy<CollectionEntry, 'timeAdded' | 'provider' | 'wantQuantity'> {
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

// True when a collection card is tracked purely as a wishlist entry — none
// owned yet, only a desired quantity.
export function isWantOnly(card: CollectionCard): boolean {
	return (card.quantity ?? 0) === 0 && (card.foilQuantity ?? 0) === 0 && (card.wantQuantity ?? 0) > 0;
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
