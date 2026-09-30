<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { app, applyTheme, setViewMode } from '$lib/state.svelte';
	import { addCollection, invalidateCache } from '$lib/api';
	import { themes } from '$lib/themes/index';
	import { isDeck } from '$lib/types';
	import type { CollectionKind } from '$lib/types';

	let { children } = $props();

	let themeOpen = $state(false);
	let addColOpen = $state(false);
	let newColName = $state('');
	let newColKind = $state<CollectionKind>('collection');
	let addColError = $state('');

	onMount(async () => {
		applyTheme(app.theme);
		await app.loadSystemInfo();
		app.loadMeta('mtg');
		if (app.collectionsEnabled) {
			await app.loadCollections();
		} else if (app.ready) {
			// Collections disabled — redirect away from collection routes
			const path = $page.url.pathname;
			if (path === '/' || path.startsWith('/collection/') || path.startsWith('/deck/')) goto('/search');
		}
	});

	// Derive current collection or deck from URL. Decks get a separate route, since their page has
	// boards rather than prices and wants.
	const currentCollection = $derived(
		$page.url.pathname.startsWith('/collection/') || $page.url.pathname.startsWith('/deck/')
			? decodeURIComponent($page.url.pathname.split('/')[2] ?? '')
			: ''
	);

	// Grouped rather than interleaved, and divided the way the tab bar already divides its other
	// groups: what is owned and what is to be played are different kinds of thing, and the deck
	// glyph alone is easy to miss in a long row.
	const collectionTabs = $derived(app.collections.filter((c) => !isDeck(c)));
	const deckTabs = $derived(app.collections.filter((c) => isDeck(c)));

	let tabBar = $state<HTMLElement | null>(null);

	// The tab bar scrolls sideways rather than wrapping, and hides its scrollbar, so a tab past
	// the right edge is reachable but invisible: land on one by URL and nothing on screen is
	// marked current. Decks share the bar, so overflowing it is now ordinary.
	$effect(() => {
		// The active tab changes with the route, and the tabs themselves don't exist until the
		// collections have loaded, so both are read here.
		if (!$page.url.pathname || app.collections.length === 0 || !tabBar) return;

		// `nearest` on both axes so an already-visible tab isn't moved, and the page doesn't jump
		// vertically to chase one that isn't.
		tabBar
			.querySelector('.tab-item.active')
			?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
	});


	function selectTheme(t: string) {
		applyTheme(t);
		themeOpen = false;
	}

	async function handleAddCollection(e: Event) {
		e.preventDefault();
		const name = newColName.trim();
		if (!name) return;
		addColError = '';
		try {
			await addCollection(name, newColKind);
			invalidateCache('collections');
			await app.loadCollections();
			const kind = newColKind;
			newColName = '';
			newColKind = 'collection';
			addColOpen = false;
			goto(`/${kind === 'deck' ? 'deck' : 'collection'}/${encodeURIComponent(name)}`);
		} catch (err) {
			addColError = String(err);
		}
	}

	function closeTheme() { themeOpen = false; }
</script>

<svelte:window onclick={(e) => {
	const t = e.target as HTMLElement;
	if (!t.closest('.theme-menu')) themeOpen = false;
	if (!t.closest('.add-collection-form') && !t.closest('.tab-item[data-add]')) addColOpen = false;
}} />

<div class="app-shell">
	<!-- ─── Header ─── -->
	<header class="main-header">
		<div class="header-top">
			<a href="/" class="header-logo brand">
				GatheRs
				<span>Collection Manager</span>
			</a>
			<div class="header-spacer"></div>

			{#if app.pendingOps.size > 0}
				<div class="ops-tracker">
					<div class="spinner"></div>
					{[...app.pendingOps.values()][0]}
				</div>
			{/if}

			<!-- View toggle -->
			<div class="view-toggle" title="Toggle view">
				<button
					class="view-toggle-btn"
					class:active={app.viewMode === 'grid'}
					onclick={() => setViewMode('grid')}
					title="Grid view"
				>
					<svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
						<rect x="0" y="0" width="6" height="6" rx="1"/>
						<rect x="8" y="0" width="6" height="6" rx="1"/>
						<rect x="0" y="8" width="6" height="6" rx="1"/>
						<rect x="8" y="8" width="6" height="6" rx="1"/>
					</svg>
				</button>
				<button
					class="view-toggle-btn"
					class:active={app.viewMode === 'list'}
					onclick={() => setViewMode('list')}
					title="List view"
				>
					<svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
						<rect x="0" y="0" width="14" height="2" rx="1"/>
						<rect x="0" y="4" width="14" height="2" rx="1"/>
						<rect x="0" y="8" width="14" height="2" rx="1"/>
						<rect x="0" y="12" width="14" height="2" rx="1"/>
					</svg>
				</button>
			</div>

			<!-- Theme switcher -->
			<div class="theme-menu">
				<button class="btn btn-ghost btn-icon" onclick={() => themeOpen = !themeOpen} title="Theme">
					<svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
						<circle cx="8" cy="8" r="3"/>
						<path d="M8 1v2M8 13v2M1 8h2M13 8h2M3.05 3.05l1.41 1.41M11.54 11.54l1.41 1.41M3.05 12.95l1.41-1.41M11.54 4.46l1.41-1.41"/>
					</svg>
				</button>
				{#if themeOpen}
					<div class="theme-dropdown">
						{#each themes as t}
							<button
								class="theme-option"
								class:active={app.theme === t.id}
								onclick={() => selectTheme(t.id)}
							>
								<span class="theme-dot" style="background: {t.dot}; border: 1px solid var(--border2)"></span>
								{t.label}
							</button>
						{/each}
					</div>
				{/if}
			</div>

			{#if !app.systemInfo?.demo_mode}
				<a href="/settings" class="btn btn-ghost btn-icon" title="Settings">
					<svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
						<path d="M8 10a2 2 0 100-4 2 2 0 000 4z"/>
						<path fill-rule="evenodd" d="M6.5 1.5h3l.5 1.5a5 5 0 011.2.7l1.5-.5 2.1 2.1-.5 1.5c.3.4.5.8.7 1.2l1.5.5v3l-1.5.5a5 5 0 01-.7 1.2l.5 1.5-2.1 2.1-1.5-.5c-.4.3-.8.5-1.2.7L9.5 14.5h-3l-.5-1.5a5 5 0 01-1.2-.7l-1.5.5-2.1-2.1.5-1.5A5 5 0 011 7.5L-.5 7V4l1.5-.5A5 5 0 011.7 2.3L1.2.8 3.3-1.3l1.5.5A5 5 0 016 .5L6.5 1.5z" clip-rule="evenodd"/>
					</svg>
				</a>
			{/if}
			<a href="https://gathers.cards" target="_blank" rel="noopener noreferrer" class="btn btn-ghost external-link" style="font-size: 0.8rem;" title="gathers.cards">
				gathers.cards
			</a>
		</div>

		<!-- ─── Tab bar ─── -->
		<div class="tab-bar" bind:this={tabBar}>
			<a href="/" class="tab-item" class:active={$page.url.pathname === '/'}>Home</a>
			<a href="/search" class="tab-item" class:active={$page.url.pathname === '/search'}>
				<svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
					<path d="M8.5 7.5l2.5 2.5M5 8.5a4 4 0 100-8 4 4 0 000 8z" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round"/>
				</svg>
				Search
			</a>

			{#if app.collectionsEnabled && collectionTabs.length > 0}
				<div class="tab-divider"></div>
				{#each collectionTabs as col}
					<a
						href="/collection/{encodeURIComponent(col.id)}"
						class="tab-item"
						class:active={currentCollection === col.id}
					>
						{col.id}
					</a>
				{/each}
			{/if}

			{#if app.collectionsEnabled && deckTabs.length > 0}
				<div class="tab-divider"></div>
				{#each deckTabs as deck}
					<a
						href="/deck/{encodeURIComponent(deck.id)}"
						class="tab-item"
						class:active={currentCollection === deck.id}
					>
						<span class="tab-kind-glyph" title="Deck">🂠</span>
						{deck.id}
					</a>
				{/each}
			{/if}

			{#if app.collectionsEnabled}
				<div class="tab-divider"></div>
				{#if addColOpen}
					<form class="add-collection-form" onsubmit={handleAddCollection}>
						<!-- svelte-ignore a11y_autofocus -->
						<input
							class="input"
							style="width: 160px; height: 28px; padding: 3px 8px; font-size: 0.82rem;"
							bind:value={newColName}
							placeholder={newColKind === 'deck' ? 'Deck name…' : 'Collection name…'}
							autofocus
						/>
						<select class="select add-collection-kind" bind:value={newColKind} title="A deck holds a list you intend to play; a collection holds what you own">
							<option value="collection">Collection</option>
							<option value="deck">Deck</option>
						</select>
						<button type="submit" class="btn btn-sm btn-accent">Add</button>
						<button type="button" class="btn btn-sm btn-ghost" onclick={() => addColOpen = false}>✕</button>
						{#if addColError}<span style="color: var(--danger); font-size: 0.75rem;">{addColError}</span>{/if}
					</form>
				{:else}
					<button data-add class="tab-item" onclick={() => addColOpen = true} title="New collection">
						<svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
							<path d="M6 1v10M1 6h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
						</svg>
						New
					</button>
				{/if}
			{/if}
		</div>

		<!-- Download progress banner -->
		{#if app.systemInfo?.downloading && Object.keys(app.systemInfo.downloading).length > 0}
			{#each Object.entries(app.systemInfo.downloading) as [sys, prog]}
				<div class="download-banner">
					<div class="spinner"></div>
					{#if prog.phase === 'downloading' && prog.total > 0}
						Downloading {sys}: {Math.round((prog.downloaded / prog.total) * 100)}%
						<div class="progress-bar-wrap">
							<div class="progress-bar-fill" style="width: {(prog.downloaded / prog.total) * 100}%"></div>
						</div>
					{:else if prog.phase === 'verifying'}
						Verifying {sys}…
					{:else}
						Checking {sys}…
					{/if}
				</div>
			{/each}
		{/if}
	</header>

	<main class="content">
		{@render children()}
	</main>
</div>
