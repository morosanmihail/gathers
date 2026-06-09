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
	sortBy: string;
	sortOrder: 'Asc' | 'Desc';
}

export interface PriceRetailer {
	normal?: number;
	foil?: number;
}

export interface CardPrices {
	paper?: Record<string, PriceRetailer>;
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
		sortBy: 'Name',
		sortOrder: 'Asc'
	};
}

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
