<script lang="ts">
	import type { SearchFilters, CardSet } from '$lib/types';
	import { app } from '$lib/state.svelte';
	import { onMount } from 'svelte';

	interface Props {
		filters: SearchFilters;
		onfilters: (f: SearchFilters) => void;
		onsubmit: () => void;
		systems?: string[];
		activeSystem?: string;
		onSystemChange?: (s: string) => void;
		compact?: boolean;
	}

	let {
		filters,
		onfilters,
		onsubmit,
		systems = [],
		activeSystem = '',
		onSystemChange,
		compact = false
	}: Props = $props();

	onMount(() => app.loadCardSets());

	function set(key: keyof SearchFilters, val: unknown) {
		onfilters({ ...filters, [key]: val });
	}

	function toggleColor(color: string) {
		const list = filters.colorIdentities.includes(color)
			? filters.colorIdentities.filter(c => c !== color)
			: [...filters.colorIdentities, color];
		set('colorIdentities', list);
	}

	function toggleDomain(d: string) {
		const list = filters.domains.includes(d)
			? filters.domains.filter(x => x !== d)
			: [...filters.domains, d];
		set('domains', list);
	}

	function toggleEnergy(e: string) {
		const list = filters.energyTypes.includes(e)
			? filters.energyTypes.filter(x => x !== e)
			: [...filters.energyTypes, e];
		set('energyTypes', list);
	}

	const setOptions = $derived(
		app.cardSets
			.filter(s => !filters.setCode || s.code.toLowerCase().startsWith(filters.setCode.toLowerCase()) || s.name.toLowerCase().includes(filters.setCode.toLowerCase()))
			.slice(0, 20)
	);

	const colors = [
		{ value: 'White', label: 'W' },
		{ value: 'Blue',  label: 'U' },
		{ value: 'Black', label: 'B' },
		{ value: 'Red',   label: 'R' },
		{ value: 'Green', label: 'G' },
	];

	// Exact enum values from APICardDomain
	const riftboundDomains = ['Calm', 'Chaos', 'Fury', 'Mind', 'Body', 'Order', 'Colorless'];

	// Exact enum values from APIEnergyType (skip 'Energy' — not useful for filtering)
	const pokemonEnergyTypes = [
		'Fire', 'Water', 'Grass', 'Lightning', 'Psychic',
		'Fighting', 'Darkness', 'Metal', 'Dragon', 'Fairy', 'Colorless',
	];

	const showMtg  = $derived(!activeSystem || activeSystem === 'Scryfall' || (activeSystem.includes('Magic') || (activeSystem.includes('Sql') && !activeSystem.includes('Rift') && !activeSystem.includes('Pokemon'))));
	const showRift = $derived(activeSystem?.includes('Riftbound') ?? false);
	const showPoke = $derived(activeSystem?.includes('Pokemon') ?? false);

	// Sort options differ by system
	const sortOptions = $derived(showRift
		? [
			{ value: 'Name',            label: 'Name' },
			{ value: 'Rarity',         label: 'Rarity' },
			{ value: 'SetCode',        label: 'Set' },
			{ value: 'CollectorNumber',label: 'Collector #' },
		]
		: showPoke
		? [
			{ value: 'Name',            label: 'Name' },
			{ value: 'Rarity',         label: 'Rarity' },
			{ value: 'SetCode',        label: 'Set' },
			{ value: 'CollectorNumber',label: 'Collector #' },
		]
		: [
			{ value: 'Name',            label: 'Name' },
			{ value: 'Rarity',         label: 'Rarity' },
			{ value: 'SetCode',        label: 'Set' },
			{ value: 'CollectorNumber',label: 'Collector #' },
			{ value: 'Artist',         label: 'Artist' },
		]);
</script>

<div class="search-panel">
	{#if !compact}<h2>Search</h2>{/if}

	<!-- System selector -->
	{#if systems.length > 1}
		<div class="checkbox-group">
			{#each systems as sys}
				<label class="chip-checkbox" class:checked={activeSystem === sys}>
					<input type="radio" name="system" value={sys} checked={activeSystem === sys}
						onchange={() => onSystemChange?.(sys)} />
					{sys.replace('SQLite','').replace('Sql','')}
				</label>
			{/each}
		</div>
	{/if}

	<form onsubmit={(e) => { e.preventDefault(); onsubmit(); }}>
		<!-- Name always shown -->
		<div class="field">
			<input class="input" placeholder="Card name…" value={filters.name}
				oninput={(e) => set('name', (e.target as HTMLInputElement).value)} />
		</div>

		<!-- Set code — all systems -->
		<div class="input-group field">
			{#if showMtg}
				<input
					class="input"
					list="set-datalist"
					placeholder="Set code…"
					value={filters.setCode}
					oninput={(e) => {
						const raw = (e.target as HTMLInputElement).value;
						const code = raw.includes(' — ') ? raw.split(' — ')[0] : raw;
						set('setCode', code);
					}}
				/>
				<datalist id="set-datalist">
					{#each setOptions as s}
						<option value="{s.code} — {s.name}"></option>
					{/each}
				</datalist>
			{:else}
				<input class="input" placeholder="Set code…" value={filters.setCode}
					oninput={(e) => set('setCode', (e.target as HTMLInputElement).value)} />
			{/if}
			<input class="input" placeholder="Collector #…" value={filters.collectorNumber}
				oninput={(e) => set('collectorNumber', (e.target as HTMLInputElement).value)}
				style="max-width: 130px" />
		</div>

		<!-- Rules text — all systems -->
		<div class="field">
			<input class="input" placeholder="Rules text…" value={filters.text}
				oninput={(e) => set('text', (e.target as HTMLInputElement).value)} />
		</div>

		<!-- MTG: artist + colors + rarity -->
		{#if showMtg}
			<div class="field">
				<input class="input" placeholder="Artist…" value={filters.artist}
					oninput={(e) => set('artist', (e.target as HTMLInputElement).value)} />
			</div>

			<div class="field">
				<span class="field-label">Color Identity</span>
				<div class="color-chips">
					{#each colors as c}
						<button
							type="button"
							class="color-chip {c.label}"
							class:active={filters.colorIdentities.includes(c.value)}
							onclick={() => toggleColor(c.value)}
							title={c.value}
						>{c.label}</button>
					{/each}
				</div>
			</div>

			<div class="field">
				<select class="input" value={filters.rarity}
					onchange={(e) => set('rarity', (e.target as HTMLSelectElement).value)}>
					<option value="">Any rarity</option>
					<option value="Common">Common</option>
					<option value="Uncommon">Uncommon</option>
					<option value="Rare">Rare</option>
					<option value="Mythic">Mythic</option>
				</select>
			</div>
		{/if}

		<!-- Riftbound: artist + domains + rarity -->
		{#if showRift}
			<div class="field">
				<input class="input" placeholder="Artist…" value={filters.artist}
					oninput={(e) => set('artist', (e.target as HTMLInputElement).value)} />
			</div>

			<div class="field">
				<span class="field-label">Domain</span>
				<div class="checkbox-group">
					{#each riftboundDomains as d}
						<label class="chip-checkbox" class:checked={filters.domains.includes(d)}>
							<input type="checkbox" value={d} checked={filters.domains.includes(d)}
								onchange={() => toggleDomain(d)} />
							{d}
						</label>
					{/each}
				</div>
			</div>

			<div class="field">
				<select class="input" value={filters.rarity}
					onchange={(e) => set('rarity', (e.target as HTMLSelectElement).value)}>
					<option value="">Any rarity</option>
					<option value="Common">Common</option>
					<option value="Uncommon">Uncommon</option>
					<option value="Rare">Rare</option>
					<option value="Mythic">Epic</option>
				</select>
			</div>
		{/if}

		<!-- Pokemon: energy types only (rarity filter broken server-side — case mismatch) -->
		{#if showPoke}
			<div class="field">
				<span class="field-label">Energy Type</span>
				<div class="checkbox-group">
					{#each pokemonEnergyTypes as e}
						<label class="chip-checkbox" class:checked={filters.energyTypes.includes(e)}>
							<input type="checkbox" value={e} checked={filters.energyTypes.includes(e)}
								onchange={() => toggleEnergy(e)} />
							{e}
						</label>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Sort — all systems -->
		<div class="input-group field">
			<select class="input" value={filters.sortBy}
				onchange={(e) => set('sortBy', (e.target as HTMLSelectElement).value)}>
				{#each sortOptions as o}
					<option value={o.value}>Sort: {o.label}</option>
				{/each}
			</select>
			<select class="input" style="max-width: 110px" value={filters.sortOrder}
				onchange={(e) => set('sortOrder', (e.target as HTMLSelectElement).value as 'Asc' | 'Desc')}>
				<option value="Asc">↑ Asc</option>
				<option value="Desc">↓ Desc</option>
			</select>
		</div>

		<button type="submit" class="btn btn-accent" style="width: 100%;">
			<svg width="14" height="14" viewBox="0 0 14 14" fill="none">
				<circle cx="6" cy="6" r="4.5" stroke="currentColor" stroke-width="1.5"/>
				<path d="M10 10l2.5 2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
			</svg>
			Search
		</button>
	</form>
</div>
