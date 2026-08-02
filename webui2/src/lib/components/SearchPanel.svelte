<script lang="ts">
	import type { SearchFilters, CardSet, TriState } from '$lib/types';
	import { legalityFormats, borderColors, colorOptions, riftboundDomains, pokemonEnergyTypes, toggleInList } from '$lib/types';
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

	let advancedOpen = $state(false);

	function toggleColor(color: string) { set('colorIdentities', toggleInList(filters.colorIdentities, color)); }
	function toggleActualColor(color: string) { set('colors', toggleInList(filters.colors, color)); }
	function toggleDomain(d: string) { set('domains', toggleInList(filters.domains, d)); }
	function toggleEnergy(e: string) { set('energyTypes', toggleInList(filters.energyTypes, e)); }

	const setOptions = $derived(
		app.cardSets
			.filter(s => !filters.setCode || s.code.toLowerCase().startsWith(filters.setCode.toLowerCase()) || s.name.toLowerCase().includes(filters.setCode.toLowerCase()))
			.slice(0, 20)
	);

	const colors = colorOptions;

	const showMtg  = $derived(!activeSystem || activeSystem === 'Scryfall' || (activeSystem.includes('Magic') || (activeSystem.includes('Sql') && !activeSystem.includes('Rift') && !activeSystem.includes('Pokemon'))));
	const showRift = $derived(activeSystem?.includes('Riftbound') ?? false);
	const showPoke = $derived(activeSystem?.includes('Pokemon') ?? false);

	// Sort options differ by system: MTG adds an Artist sort the others don't have
	const baseSortOptions = [
		{ value: 'Name',             label: 'Name' },
		{ value: 'Rarity',           label: 'Rarity' },
		{ value: 'SetCode',          label: 'Set' },
		{ value: 'CollectorNumber',  label: 'Collector #' }
	];
	const sortOptions = $derived(
		showPoke
			? [...baseSortOptions, { value: 'ReleaseDate', label: 'Release date' }]
			: showRift
				? baseSortOptions
				: [...baseSortOptions, { value: 'Artist', label: 'Artist' }]
	);
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

		<!-- Rules text — all systems (Pokemon: matches card description) -->
		<div class="field">
			<input class="input" placeholder={showPoke ? 'Card description…' : 'Rules text…'} value={filters.text}
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

			<div class="field">
				<button type="button" class="advanced-toggle" onclick={() => advancedOpen = !advancedOpen}>
					<span class="advanced-toggle-icon" class:open={advancedOpen}>▸</span>
					Advanced filters
				</button>
			</div>

			{#if advancedOpen}
				<div class="advanced-filters">
					<div class="input-group field">
						<input class="input" type="number" step="0.5" min="0" placeholder="Min mana value" value={filters.manaValueMin}
							oninput={(e) => set('manaValueMin', (e.target as HTMLInputElement).value)} />
						<input class="input" type="number" step="0.5" min="0" placeholder="Max mana value" value={filters.manaValueMax}
							oninput={(e) => set('manaValueMax', (e.target as HTMLInputElement).value)} />
					</div>

					<div class="field">
						<span class="field-label">Colors (printed)</span>
						<div class="color-chips">
							{#each colors as c}
								<button
									type="button"
									class="color-chip {c.label}"
									class:active={filters.colors.includes(c.value)}
									onclick={() => toggleActualColor(c.value)}
									title={c.value}
								>{c.label}</button>
							{/each}
						</div>
					</div>

					<div class="input-group field">
						<input class="input" placeholder="Power…" value={filters.power}
							oninput={(e) => set('power', (e.target as HTMLInputElement).value)} />
						<input class="input" placeholder="Toughness…" value={filters.toughness}
							oninput={(e) => set('toughness', (e.target as HTMLInputElement).value)} />
					</div>

					<div class="input-group field">
						<input class="input" placeholder="Loyalty…" value={filters.loyalty}
							oninput={(e) => set('loyalty', (e.target as HTMLInputElement).value)} />
						<input class="input" placeholder="Defense…" value={filters.defense}
							oninput={(e) => set('defense', (e.target as HTMLInputElement).value)} />
					</div>

					<div class="field">
						<input class="input" placeholder="Keywords… (e.g. Flying, Trample)" value={filters.keywords}
							oninput={(e) => set('keywords', (e.target as HTMLInputElement).value)} />
					</div>

					<div class="input-group field">
						<select class="input" value={filters.borderColor}
							onchange={(e) => set('borderColor', (e.target as HTMLSelectElement).value)}>
							<option value="">Any border</option>
							{#each borderColors as b}
								<option value={b}>{b[0].toUpperCase() + b.slice(1)}</option>
							{/each}
						</select>
						<select class="input" value={filters.legalIn}
							onchange={(e) => set('legalIn', (e.target as HTMLSelectElement).value)}>
							<option value="">Legal in any format</option>
							{#each legalityFormats as f}
								<option value={f.value}>{f.label}</option>
							{/each}
						</select>
					</div>

					<div class="input-group field">
						<select class="input" value={filters.isReserved}
							onchange={(e) => set('isReserved', (e.target as HTMLSelectElement).value as TriState)}>
							<option value="">Reserved: any</option>
							<option value="true">Reserved: yes</option>
							<option value="false">Reserved: no</option>
						</select>
						<select class="input" value={filters.isPromo}
							onchange={(e) => set('isPromo', (e.target as HTMLSelectElement).value as TriState)}>
							<option value="">Promo: any</option>
							<option value="true">Promo: yes</option>
							<option value="false">Promo: no</option>
						</select>
					</div>

					<div class="input-group field">
						<select class="input" value={filters.isReprint}
							onchange={(e) => set('isReprint', (e.target as HTMLSelectElement).value as TriState)}>
							<option value="">Reprint: any</option>
							<option value="true">Reprint: yes</option>
							<option value="false">Reprint: no</option>
						</select>
						<select class="input" value={filters.isFullArt}
							onchange={(e) => set('isFullArt', (e.target as HTMLSelectElement).value as TriState)}>
							<option value="">Full art: any</option>
							<option value="true">Full art: yes</option>
							<option value="false">Full art: no</option>
						</select>
					</div>
				</div>
			{/if}
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

		<!-- Pokemon: energy types + pokedex # (rarity filter broken server-side — case mismatch) -->
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

			<div class="field">
				<input class="input" type="number" min="1" placeholder="National Pokédex #" value={filters.pokedex}
					oninput={(e) => set('pokedex', (e.target as HTMLInputElement).value)} />
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

<style>
	.advanced-toggle {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		background: none;
		border: none;
		padding: 4px 0;
		color: var(--text2);
		font-size: 0.78rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		cursor: pointer;
	}

	.advanced-toggle:hover { color: var(--accent-text); }

	.advanced-toggle-icon {
		display: inline-block;
		transition: transform 0.12s;
	}

	.advanced-toggle-icon.open { transform: rotate(90deg); }

	.advanced-filters {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--border);
		padding-top: 10px;
		margin-bottom: 4px;
	}
</style>
