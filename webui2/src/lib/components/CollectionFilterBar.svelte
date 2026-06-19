<script lang="ts">
	import type { SearchFilters } from '$lib/types';
	import { app } from '$lib/state.svelte';

	interface Props {
		filters: SearchFilters;
		onfilters: (f: SearchFilters) => void;
		onclose: () => void;
	}

	let { filters, onfilters, onclose }: Props = $props();

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

	const showMtg  = $derived(app.systems.some(s => s === 'Sql' || s === 'Scryfall'));
	const showRift = $derived(app.systems.some(s => s.includes('Riftbound')));
	const showPoke = $derived(app.systems.some(s => s.includes('Pokemon')));

	const colors = [
		{ value: 'White', label: 'W' },
		{ value: 'Blue',  label: 'U' },
		{ value: 'Black', label: 'B' },
		{ value: 'Red',   label: 'R' },
		{ value: 'Green', label: 'G' },
	];

	const domains = ['Calm', 'Chaos', 'Fury', 'Mind', 'Body', 'Order', 'Colorless'];

	const energyTypes = [
		'Fire', 'Water', 'Grass', 'Lightning', 'Psychic',
		'Fighting', 'Darkness', 'Metal', 'Dragon', 'Fairy', 'Colorless',
	];
</script>

<div class="cfilter-bar">
	<div class="cfilter-row">
		<input
			class="input cfilter-input"
			placeholder="Name…"
			value={filters.name}
			oninput={(e) => set('name', (e.target as HTMLInputElement).value)}
		/>
		<input
			class="input cfilter-input"
			placeholder="Set…"
			value={filters.setCode}
			oninput={(e) => set('setCode', (e.target as HTMLInputElement).value)}
			style="max-width: 80px"
		/>
		<input
			class="input cfilter-input"
			placeholder="Rules text…"
			value={filters.text}
			oninput={(e) => set('text', (e.target as HTMLInputElement).value)}
		/>
		{#if showMtg || showRift}
			<input
				class="input cfilter-input"
				placeholder="Artist…"
				value={filters.artist}
				oninput={(e) => set('artist', (e.target as HTMLInputElement).value)}
			/>
		{/if}
		<select
			class="input cfilter-input"
			value={filters.rarity}
			onchange={(e) => set('rarity', (e.target as HTMLSelectElement).value)}
			style="max-width: 130px"
		>
			<option value="">Any rarity</option>
			<option value="Common">Common</option>
			<option value="Uncommon">Uncommon</option>
			<option value="Rare">Rare</option>
			<option value="Mythic">{showRift ? 'Epic' : 'Mythic'}</option>
		</select>

		{#if showMtg}
			<div class="cfilter-sep"></div>
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
		{/if}

		{#if showRift}
			<div class="cfilter-sep"></div>
			<div class="cfilter-domain-chips">
				{#each domains as d}
					<button
						type="button"
						class="cfilter-domain-chip"
						class:active={filters.domains.includes(d)}
						onclick={() => toggleDomain(d)}
						title={d}
					>{d.slice(0, 2)}</button>
				{/each}
			</div>
		{/if}

		{#if showPoke}
			<div class="cfilter-sep"></div>
			<div class="cfilter-energy-chips">
				{#each energyTypes as e}
					<label class="chip-checkbox" class:checked={filters.energyTypes.includes(e)}>
						<input type="checkbox" checked={filters.energyTypes.includes(e)} onchange={() => toggleEnergy(e)} />
						{e}
					</label>
				{/each}
			</div>
		{/if}

		<div style="margin-left: auto; display: flex; gap: 6px; align-items: center;">
			<button type="button" class="btn btn-ghost btn-sm" onclick={onclose} title="Close filter bar">
				<svg width="12" height="12" viewBox="0 0 12 12" fill="none">
					<path d="M1 1l10 10M11 1L1 11" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
				</svg>
				Close
			</button>
		</div>
	</div>
</div>

<style>
	.cfilter-bar {
		padding: 8px 20px;
		background: var(--bg);
		border-bottom: 1px solid var(--border);
	}

	.cfilter-row {
		display: flex;
		gap: 8px;
		align-items: center;
		flex-wrap: wrap;
	}

	.cfilter-input {
		height: 32px;
		padding: 4px 10px;
		font-size: 0.82rem;
		flex: 1;
		min-width: 80px;
		max-width: 180px;
	}

	.cfilter-sep {
		width: 1px;
		height: 24px;
		background: var(--border);
		flex-shrink: 0;
	}

	.cfilter-domain-chips {
		display: flex;
		gap: 4px;
	}

	.cfilter-domain-chip {
		width: 30px;
		height: 30px;
		border-radius: 6px;
		border: 2px solid var(--border2);
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		font-size: 0.68rem;
		font-weight: 800;
		transition: all 0.12s;
		background: transparent;
		color: var(--text2);
		letter-spacing: -0.5px;
	}

	.cfilter-domain-chip:not(.active) { opacity: 0.45; }
	.cfilter-domain-chip:hover { opacity: 1; }
	.cfilter-domain-chip.active {
		box-shadow: 0 0 0 3px var(--accent-glow);
		transform: scale(1.1);
		opacity: 1;
		background: var(--accent-glow);
		border-color: var(--accent-text);
		color: var(--accent-text);
	}

	.cfilter-energy-chips {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
	}
</style>
