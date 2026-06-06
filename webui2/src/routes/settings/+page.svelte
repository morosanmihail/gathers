<script lang="ts">
	import { onMount } from 'svelte';
	import { getSettings, saveSettings, triggerUpdate, invalidateSystemInfo } from '$lib/api';
	import { app } from '$lib/state.svelte';
	import type { Settings } from '$lib/types';

	let config = $state<Settings | null>(null);
	let error = $state('');
	let saving = $state(false);
	let saved = $state(false);
	let demoMode = $state(false);

	const SYSTEM_LABELS: Record<string, string> = {
		Sql: 'Magic: The Gathering (SQLite)',
		Scryfall: 'Magic: The Gathering (Scryfall)',
		RiftboundSql: 'Riftbound (SQLite)',
		PokemonSql: 'Pokémon (SQLite)',
	};

	const ALL_SYSTEMS = ['Sql', 'Scryfall', 'RiftboundSql', 'PokemonSql'];

	const SYSTEM_ACTIONS: Record<string, Array<{ label: string; endpoint: string }>> = {
		Sql: [
			{ label: 'Update DB',     endpoint: '/api/mtg/update' },
			{ label: 'Update Prices', endpoint: '/api/mtg/prices/update' },
		],
		RiftboundSql: [
			{ label: 'Update DB', endpoint: '/api/riftbound/update' },
		],
		PokemonSql: [
			{ label: 'Update DB',     endpoint: '/api/pokemon/update' },
			{ label: 'Update Prices', endpoint: '/api/pokemon/prices/update' },
		],
	};

	const PATH_FIELDS = [
		{ key: 'mtg_db_path',         label: 'MTG Database path' },
		{ key: 'mtg_prices_path',     label: 'MTG Prices path' },
		{ key: 'riftbound_db_path',   label: 'Riftbound Database path' },
		{ key: 'pokemon_db_path',     label: 'Pokémon Database path' },
		{ key: 'pokemon_prices_path', label: 'Pokémon Prices path' },
		{ key: 'storage_db_path',     label: 'Storage Database path' },
	];

	// Per-button update state
	let updateStates = $state<Record<string, { running: boolean; msg: string; ok: boolean }>>({});

	onMount(async () => {
		try {
			const res = await fetch('/api/settings');
			if (res.status === 403) { demoMode = true; return; }
			if (!res.ok) throw new Error(`Failed (${res.status})`);
			config = await res.json();
		} catch (e) {
			error = String(e);
		}
	});

	function toggleSystem(sys: string) {
		if (!config) return;
		const has = config.system.includes(sys);
		config = { ...config, system: has ? config.system.filter(s => s !== sys) : [...config.system, sys] };
		saved = false;
	}

	function setPath(key: string, val: string) {
		if (!config) return;
		config = { ...config, [key]: val || null };
		saved = false;
	}

	async function handleSave() {
		if (!config) return;
		saving = true; saved = false; error = '';
		try {
			config = await saveSettings(config);
			saved = true;
			invalidateSystemInfo();
			await app.loadSystemInfo();
		} catch (e) {
			error = String(e);
		} finally {
			saving = false;
		}
	}

	async function runUpdate(key: string, endpoint: string) {
		updateStates = { ...updateStates, [key]: { running: true, msg: '', ok: false } };
		try {
			const msg = await triggerUpdate(endpoint);
			updateStates = { ...updateStates, [key]: { running: false, msg, ok: true } };
		} catch (e) {
			updateStates = { ...updateStates, [key]: { running: false, msg: String(e), ok: false } };
		}
		setTimeout(() => {
			const s = { ...updateStates };
			delete s[key];
			updateStates = s;
		}, 4000);
	}
</script>

<div>
	<div class="page-header">
		<h1 class="page-title">Settings</h1>
	</div>

	<div style="padding: 0 20px 40px; max-width: 720px;">
		{#if demoMode}
			<div style="background: var(--surface2); border: 1px solid var(--border2); border-radius: var(--radius); padding: 14px 16px; margin-bottom: 20px; color: var(--text2);">
				Settings are disabled in demo mode.
			</div>
		{/if}

		{#if error}
			<div style="background: color-mix(in srgb, var(--danger) 15%, transparent); border: 1px solid var(--danger); border-radius: var(--radius); padding: 14px 16px; margin-bottom: 20px; color: var(--danger);">
				{error}
			</div>
		{/if}

		{#if !demoMode && !config && !error}
			<div class="loading-row"><div class="spinner"></div> Loading settings…</div>
		{/if}

		{#if config}
			<!-- Systems -->
			<div style="background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); margin-bottom: 20px; overflow: hidden;">
				<div style="padding: 12px 16px; border-bottom: 1px solid var(--border); font-size: 0.78rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: var(--text2);">
					Systems
				</div>
				<div style="padding: 16px;">
					{#each ALL_SYSTEMS as sys}
						{@const actions = SYSTEM_ACTIONS[sys]}
						<div style="display: flex; align-items: center; gap: 12px; margin-bottom: 12px;">
							<label style="display: flex; align-items: center; gap: 8px; cursor: pointer; flex: 1;">
								<input type="checkbox" checked={config.system.includes(sys)} onchange={() => toggleSystem(sys)} style="width: 16px; height: 16px; accent-color: var(--accent);" />
								<span>{SYSTEM_LABELS[sys] ?? sys}</span>
							</label>
							{#if actions && config.system.includes(sys)}
								<div style="display: flex; gap: 6px; flex-wrap: wrap;">
									{#each actions as action}
										{@const key = sys + action.endpoint}
										{@const st = updateStates[key]}
										<button
											class="btn btn-sm"
											disabled={st?.running}
											onclick={() => runUpdate(key, action.endpoint)}
										>
											{st?.running ? '…' : action.label}
										</button>
										{#if st && !st.running}
											<span style="font-size: 0.75rem; color: {st.ok ? 'var(--success)' : 'var(--danger)'};">
												{st.msg}
											</span>
										{/if}
									{/each}
								</div>
							{/if}
						</div>
					{/each}
				</div>
			</div>

			<!-- Server -->
			<div style="background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); margin-bottom: 20px; overflow: hidden;">
				<div style="padding: 12px 16px; border-bottom: 1px solid var(--border); font-size: 0.78rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: var(--text2);">
					Server
				</div>
				<div style="padding: 16px;">
					<label class="field-label">Port</label>
					<input
						type="number"
						class="input"
						style="max-width: 120px;"
						value={config.port}
						onchange={(e) => { if (config) { config = { ...config, port: parseInt((e.target as HTMLInputElement).value) || config.port }; saved = false; } }}
					/>
				</div>
			</div>

			<!-- Features -->
			<div style="background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); margin-bottom: 20px; overflow: hidden;">
				<div style="padding: 12px 16px; border-bottom: 1px solid var(--border); font-size: 0.78rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: var(--text2);">
					Features
				</div>
				<div style="padding: 16px; display: flex; flex-direction: column; gap: 14px;">
					<label style="display: flex; align-items: center; gap: 10px; cursor: pointer;">
						<input type="checkbox" checked={config.collections_enabled ?? true}
							onchange={() => { if (config) { config = { ...config, collections_enabled: !(config.collections_enabled ?? true) }; saved = false; } }}
							style="width: 16px; height: 16px; accent-color: var(--accent);" />
						<div>
							<div style="font-weight: 600;">Enable collections</div>
							<div style="font-size: 0.8rem; color: var(--text2);">Track owned cards across named collections</div>
						</div>
					</label>
					<label style="display: flex; align-items: center; gap: 10px; cursor: pointer;">
						<input type="checkbox" checked={config.pricing_enabled ?? true}
							onchange={() => { if (config) { config = { ...config, pricing_enabled: !(config.pricing_enabled ?? true) }; saved = false; } }}
							style="width: 16px; height: 16px; accent-color: var(--accent);" />
						<div>
							<div style="font-weight: 600;">Enable pricing</div>
							<div style="font-size: 0.8rem; color: var(--text2);">Show market prices and purchase history</div>
						</div>
					</label>
				</div>
			</div>

			<!-- File paths -->
			<div style="background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-lg); margin-bottom: 24px; overflow: hidden;">
				<div style="padding: 12px 16px; border-bottom: 1px solid var(--border); font-size: 0.78rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: var(--text2);">
					File Paths
				</div>
				<div style="padding: 16px; display: flex; flex-direction: column; gap: 12px;">
					{#each PATH_FIELDS as { key, label }}
						<div>
							<label class="field-label">{label}</label>
							<input
								type="text"
								class="input mono"
								value={(config as Record<string, unknown>)[key] as string ?? ''}
								oninput={(e) => setPath(key, (e.target as HTMLInputElement).value)}
								placeholder="(default)"
							/>
						</div>
					{/each}
				</div>
			</div>

			<div style="display: flex; align-items: center; gap: 12px;">
				<button class="btn btn-accent" onclick={handleSave} disabled={saving || demoMode}>
					{saving ? 'Saving…' : 'Save Settings'}
				</button>
				{#if saved}
					<span style="color: var(--success); font-size: 0.85rem;">
						Saved. Port/path changes need a server restart.
					</span>
				{/if}
			</div>
		{/if}
	</div>
</div>
