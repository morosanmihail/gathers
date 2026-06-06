import type { Collection, SystemInfo, Theme, ViewMode, CardSet } from './types';
import { listCollections, getSystemInfo, getMtgCardSets } from './api';

class AppState {
	collections = $state<Collection[]>([]);
	systemInfo = $state<SystemInfo | null>(null);
	ready = $state(false);
	theme = $state<Theme>('dark');
	viewMode = $state<ViewMode>('grid');
	cardSets = $state<CardSet[]>([]);
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

	async loadCardSets() {
		if (this.cardSets.length) return;
		try {
			this.cardSets = await getMtgCardSets();
		} catch {
			/* ignore */
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

// Persist theme to localStorage
if (typeof localStorage !== 'undefined') {
	const saved = localStorage.getItem('gathers-theme') as Theme | null;
	if (saved) app.theme = saved;
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
