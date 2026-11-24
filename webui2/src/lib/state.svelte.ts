import type { Collection, SystemInfo, Theme, ViewMode, CardSet, UniqueMode, GameKey, SystemMeta } from './types';
import { listCollections, getSystemInfo, getSystemMeta } from './api';
import { injectThemeStyles } from './themes/index';

// Art view zoom: the card width, in px. Below the minimum the art is too
// small to recognise; above the maximum a single card no longer fits a phone.
export const ART_ZOOM_MIN = 90;
export const ART_ZOOM_MAX = 320;
export const ART_ZOOM_DEFAULT = 150;
export const ART_ZOOM_STEP = 10;

class AppState {
	collections = $state<Collection[]>([]);
	systemInfo = $state<SystemInfo | null>(null);
	ready = $state(false);
	theme = $state<Theme>('dark');
	viewMode = $state<ViewMode>('grid');
	/** Art view card width in px; see `setArtZoom`. */
	artZoom = $state(ART_ZOOM_DEFAULT);
	// Each game's search metadata (sets, card types, grouping modes), loaded on
	// first use by `loadMeta`.
	meta = $state<Partial<Record<GameKey, SystemMeta>>>({});
	selectedCards = $state<Set<string>>(new Set());
	pendingOps = $state<Map<string, string>>(new Map());

	get collectionsEnabled() {
		return this.systemInfo?.collections_enabled ?? false;
	}

	get pricingEnabled() {
		return this.systemInfo?.pricing_enabled ?? false;
	}

	get systems() {
		return this.systemInfo?.systems ?? [];
	}

	get plugins() {
		return this.systemInfo?.plugins ?? [];
	}

	/** The ways `system` can collapse search results that share a card, default
	 *  first. Empty when it has nothing to toggle (Riftbound, Pokemon, plugins). */
	uniqueModes(system: string): UniqueMode[] {
		return this.systemInfo?.unique_modes?.[system] ?? [];
	}

	async loadSystemInfo() {
		try {
			this.systemInfo = await getSystemInfo();
		} catch {
			/* server not ready */
		} finally {
			this.ready = true;
		}
	}

	async loadCollections() {
		if (!this.collectionsEnabled) return;
		try {
			this.collections = await listCollections();
		} catch {
			/* ignore */
		}
	}

	get cardSets(): CardSet[] {
		return this.meta.mtg?.sets ?? [];
	}

	/** Sets of `game`, empty until `loadMeta(game)` has finished. */
	sets(game: GameKey): CardSet[] {
		return this.meta[game]?.sets ?? [];
	}

	/** Card types `game` can be filtered by, empty until `loadMeta(game)` has finished. */
	cardTypes(game: GameKey): string[] {
		return this.meta[game]?.card_types ?? [];
	}

	async loadMeta(game: GameKey) {
		if (this.meta[game]) return;
		try {
			const meta = await getSystemMeta(game);
			this.meta = { ...this.meta, [game]: meta };
		} catch {
			/* game not configured, or server not ready */
		}
	}

	addOp(id: string, label: string) {
		const m = new Map(this.pendingOps);
		m.set(id, label);
		this.pendingOps = m;
	}

	removeOp(id: string) {
		const m = new Map(this.pendingOps);
		m.delete(id);
		this.pendingOps = m;
	}

	async withOp<T>(label: string, fn: () => Promise<T>): Promise<T> {
		const id = Math.random().toString(36).slice(2);
		this.addOp(id, label);
		try {
			return await fn();
		} finally {
			this.removeOp(id);
		}
	}

	toggleSelected(id: string) {
		const s = new Set(this.selectedCards);
		if (s.has(id)) s.delete(id);
		else s.add(id);
		this.selectedCards = s;
	}

	clearSelected() {
		this.selectedCards = new Set();
	}

	selectAll(ids: string[]) {
		this.selectedCards = new Set(ids);
	}
}

export const app = new AppState();

// Inject all theme CSS rules and restore saved theme
if (typeof document !== 'undefined') {
	injectThemeStyles();
}
if (typeof localStorage !== 'undefined') {
	const saved = localStorage.getItem('gathers-theme') as Theme | null;
	if (saved) {
		app.theme = saved;
		document.documentElement.setAttribute('data-theme', saved);
	}
}

export function applyTheme(theme: Theme) {
	app.theme = theme;
	if (typeof localStorage !== 'undefined') {
		localStorage.setItem('gathers-theme', theme);
	}
	if (typeof document !== 'undefined') {
		document.documentElement.setAttribute('data-theme', theme);
	}
}

const clampZoom = (px: number) =>
	Math.min(ART_ZOOM_MAX, Math.max(ART_ZOOM_MIN, Math.round(px / ART_ZOOM_STEP) * ART_ZOOM_STEP));

try {
	const saved = Number(localStorage.getItem('gathers-art-zoom'));
	if (saved) app.artZoom = clampZoom(saved);
} catch { /* no storage: keep the default */ }

/** CSS variables for an art view at the current zoom: the card width, and
 *  the scale relative to the default that badges, gaps and stack offsets
 *  follow. */
export function artZoomStyle(): string {
	return `--art-card-w: ${app.artZoom}px; --art-scale: ${app.artZoom / ART_ZOOM_DEFAULT};`;
}

export function setArtZoom(px: number) {
	app.artZoom = clampZoom(px);
	try { localStorage.setItem('gathers-art-zoom', String(app.artZoom)); } catch { /* not persisted */ }
}

// Persist view mode
if (typeof localStorage !== 'undefined') {
	const saved = localStorage.getItem('gathers-view') as ViewMode | null;
	if (saved) app.viewMode = saved;
}

export function setViewMode(mode: ViewMode) {
	app.viewMode = mode;
	if (typeof localStorage !== 'undefined') {
		localStorage.setItem('gathers-view', mode);
	}
}
