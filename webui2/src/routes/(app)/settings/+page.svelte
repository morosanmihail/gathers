<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { saveSettings, triggerUpdate, restartServer, invalidateSystemInfo } from '$lib/api';
	import { app } from '$lib/state.svelte';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import type { PluginConfig, Settings, System } from '$lib/types';

	let config = $state<Settings | null>(null);
	let error = $state('');
	let saveState = $state<'idle' | 'saving' | 'saved'>('idle');
	let demoMode = $state(false);
	let restarting = $state(false);
	let confirmRestart = $state(false);

	// Set by the server when a save changed something that only applies after
	// a restart; the restart itself clears it.
	const restartRequired = $derived(app.systemInfo?.restart_required ?? false);

	const SYSTEM_LABELS: Record<string, string> = {
		Sql: 'Magic: The Gathering (SQLite)',
		Scryfall: 'Magic: The Gathering (Scryfall)',
		RiftboundSql: 'Riftbound (SQLite)',
		PokemonSql: 'Pokémon (SQLite)',
	};

	const ALL_SYSTEMS: System[] = ['Sql', 'Scryfall', 'RiftboundSql', 'PokemonSql'];

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
		// The cached system info may predate a restart-needing save.
		invalidateSystemInfo();
		app.loadSystemInfo();
		try {
			const res = await fetch('/api/settings');
			if (res.status === 403) { demoMode = true; return; }
			if (!res.ok) throw new Error(`Failed (${res.status})`);
			config = await res.json();
		} catch (e) {
			error = String(e);
		}
	});

	function toggleSystem(sys: System) {
		if (!config) return;
		const has = config.system.includes(sys);
		config = { ...config, system: has ? config.system.filter(s => s !== sys) : [...config.system, sys] };
		queueSave();
	}

	function setPath(key: string, val: string) {
		if (!config) return;
		config = { ...config, [key]: val || null };
		queueSave(TYPING_DEBOUNCE_MS);
	}

	function addPlugin() {
		if (!config) return;
		const plugins: PluginConfig[] = [...(config.plugins ?? []), { name: '', base_url: '', enabled: true }];
		config = { ...config, plugins };
		queueSave();
	}

	function updatePlugin(index: number, patch: Partial<PluginConfig>) {
		if (!config) return;
		const plugins = [...(config.plugins ?? [])];
		plugins[index] = { ...plugins[index], ...patch };
		config = { ...config, plugins };
		queueSave('name' in patch || 'base_url' in patch ? TYPING_DEBOUNCE_MS : 0);
	}

	function removePlugin(index: number) {
		if (!config) return;
		config = { ...config, plugins: (config.plugins ?? []).filter((_, i) => i !== index) };
		queueSave();
	}

	// Two plugins sharing a name would collide on `/api/plugins/{name}/...`
	// and on the `plugin-{name}` provider string used when a card is added
	// to a collection — block saving a config with duplicates from here,
	// rather than let the web UI itself create the ambiguity. (A duplicate
	// hand-edited directly into server.toml is a different case — the
	// server logs a warning and keeps only the last one.)
	const duplicatePluginNames = $derived.by(() => {
		const seen = new Set<string>();
		const dupes = new Set<string>();
		for (const p of config?.plugins ?? []) {
			const name = p.name.trim();
			if (!name) continue;
			if (seen.has(name)) dupes.add(name);
			seen.add(name);
		}
		return dupes;
	});

	// Every edit saves itself: toggles and numbers right away, text fields once
	// typing pauses (or the field loses focus). `dirty` means there are edits
	// not yet sent; only one save runs at a time, and edits made while it's in
	// flight trigger another as soon as it finishes.
	const TYPING_DEBOUNCE_MS = 800;
	let saveTimer: ReturnType<typeof setTimeout> | undefined;
	let saveInFlight = false;
	let dirty = false;

	function queueSave(delayMs = 0) {
		dirty = true;
		clearTimeout(saveTimer);
		saveTimer = setTimeout(flushSave, delayMs);
	}

	// A plugin row still missing its name or URL is a draft the user is midway
	// through filling in — it's left out of what's saved, not rejected.
	function savableConfig(c: Settings): Settings {
		return { ...c, plugins: (c.plugins ?? []).filter(p => p.name.trim() && p.base_url.trim()) };
	}

	async function flushSave() {
		clearTimeout(saveTimer);
		if (!config || !dirty || saveInFlight) return;
		if (duplicatePluginNames.size > 0) {
			error = `Duplicate plugin name(s): ${[...duplicatePluginNames].join(', ')}. Names must be unique — not saved.`;
			saveState = 'idle';
			return;
		}
		// Cleared before sending: an edit that lands mid-save sets it again.
		// A failed save leaves it cleared, so it retries on the next edit
		// rather than in a loop.
		dirty = false;
		saveInFlight = true;
		saveState = 'saving';
		error = '';
		try {
			// Keep the local `config` as is rather than replacing it with the
			// response — it may already hold newer edits.
			await saveSettings(savableConfig($state.snapshot(config) as Settings));
			saveState = dirty ? 'saving' : 'saved';
			invalidateSystemInfo();
			await app.loadSystemInfo();
		} catch (e) {
			saveState = 'idle';
			error = `Not saved: ${e}`;
		} finally {
			saveInFlight = false;
			if (dirty) flushSave();
		}
	}

	// Leaving the page shouldn't drop an edit still waiting out its debounce.
	onDestroy(() => {
		if (dirty) flushSave();
	});

	async function handleRestart() {
		confirmRestart = false;
		restarting = true; error = '';
		try {
			await restartServer();
			// The restarted server starts with the flag cleared.
			invalidateSystemInfo();
			await app.loadSystemInfo();
		} catch (e) {
			error = String(e);
		} finally {
			restarting = false;
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

{#if confirmRestart}
	<ConfirmDialog
		title="Restart server?"
		message={Object.keys(app.systemInfo?.downloading ?? {}).length > 0
			? 'The server will be briefly unavailable, and database downloads in progress will be interrupted.'
			: 'The server will be briefly unavailable while it restarts.'}
		confirmLabel="Restart"
		onconfirm={handleRestart}
		oncancel={() => (confirmRestart = false)}
	/>
{/if}

<svelte:head>
	<title>Settings - gatheRs</title>
</svelte:head>

<div>
	<div class="page-header">
		<h1 class="page-title">Settings</h1>
		<span class="save-status" role="status" aria-live="polite">
			{#if saveState === 'saving'}Saving…{:else if saveState === 'saved'}All changes saved{/if}
		</span>
	</div>

	<div class="settings-page">
		{#if restartRequired && !demoMode}
			<div class="restart-banner" role="status">
				<span>
					{restarting ? 'Restarting the server…' : 'Some saved changes only take effect after a server restart.'}
				</span>
				<button class="btn btn-accent btn-sm" onclick={() => (confirmRestart = true)} disabled={restarting}>
					{restarting ? 'Restarting…' : 'Restart now'}
				</button>
			</div>
		{/if}

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
			<div class="settings-grid">
				<div class="settings-col">
					<!-- Systems -->
					<div class="panel">
						<div class="panel-title">
							Systems
						</div>
						<div style="padding: 16px;">
							{#each ALL_SYSTEMS as sys}
								{@const actions = SYSTEM_ACTIONS[sys]}
								<div style="display: flex; align-items: center; gap: 12px; margin-bottom: 12px; flex-wrap: wrap;">
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

					<!-- Plugins -->
					<div class="panel">
						<div class="panel-title">
							Plugins
						</div>
						<div style="padding: 16px; display: flex; flex-direction: column; gap: 12px;">
							{#if (config.plugins ?? []).length === 0}
								<div style="font-size: 0.85rem; color: var(--text2);">No plugins configured.</div>
							{/if}
							{#each config.plugins ?? [] as plugin, i}
								<div style="display: flex; gap: 10px; align-items: flex-end; flex-wrap: wrap; padding: 10px; border: 1px solid var(--border); border-radius: var(--radius);">
									<div style="flex: 1; min-width: 140px;">
										<label class="field-label" for="plugin-name-{i}">Name</label>
										<input
											id="plugin-name-{i}"
											type="text"
											class="input"
											value={plugin.name}
											oninput={(e) => updatePlugin(i, { name: (e.target as HTMLInputElement).value })}
											onchange={flushSave}
											placeholder="my-plugin"
										/>
										{#if duplicatePluginNames.has(plugin.name.trim())}
											<div style="font-size: 0.75rem; color: var(--danger); margin-top: 4px;">Duplicate name</div>
										{/if}
									</div>
									<div style="flex: 2; min-width: 220px;">
										<label class="field-label" for="plugin-url-{i}">Base URL</label>
										<input
											id="plugin-url-{i}"
											type="text"
											class="input mono"
											value={plugin.base_url}
											oninput={(e) => updatePlugin(i, { base_url: (e.target as HTMLInputElement).value })}
											onchange={flushSave}
											placeholder="http://localhost:5236"
										/>
									</div>
									<label style="display: flex; align-items: center; gap: 6px; cursor: pointer; padding-bottom: 8px;">
										<input
											type="checkbox"
											checked={plugin.enabled}
											onchange={() => updatePlugin(i, { enabled: !plugin.enabled })}
											style="width: 16px; height: 16px; accent-color: var(--accent);"
										/>
										Enabled
									</label>
									{#if plugin.name && plugin.enabled}
										{@const key = 'plugin:' + plugin.name}
										{@const st = updateStates[key]}
										<button
											class="btn btn-sm"
											disabled={st?.running}
											onclick={() => runUpdate(key, `/api/plugins/${encodeURIComponent(plugin.name)}/update`)}
										>
											{st?.running ? '…' : 'Update'}
										</button>
										{#if st && !st.running}
											<span style="font-size: 0.75rem; color: {st.ok ? 'var(--success)' : 'var(--danger)'};">
												{st.msg}
											</span>
										{/if}
									{/if}
									<button class="btn btn-sm" onclick={() => removePlugin(i)}>Remove</button>
								</div>
							{/each}
							<button class="btn btn-sm" style="align-self: flex-start;" onclick={addPlugin}>+ Add plugin</button>
							<div style="font-size: 0.8rem; color: var(--text2);">Changes to plugins require a server restart.</div>
						</div>
				</div>

					<!-- Auto-download -->
					<div class="panel">
						<div class="panel-title">
							Auto-Download
						</div>
						<div style="padding: 16px; display: flex; flex-direction: column; gap: 14px;">
							<label style="display: flex; align-items: center; gap: 10px; cursor: pointer;">
								<input type="checkbox" checked={config.auto_download_enabled ?? false}
									onchange={() => { if (config) { config = { ...config, auto_download_enabled: !(config.auto_download_enabled ?? false) }; queueSave(); } }}
									style="width: 16px; height: 16px; accent-color: var(--accent);" />
								<div>
									<div style="font-weight: 600;">Enable periodic auto-download</div>
									<div style="font-size: 0.8rem; color: var(--text2);">Automatically re-download card and price databases for all active systems on a schedule</div>
								</div>
							</label>
							<div>
								<label class="field-label" for="settings-auto-download-interval">Interval (hours)</label>
								<input
									id="settings-auto-download-interval"
									type="number"
									min="1"
									class="input"
									style="max-width: 120px;"
									value={config.auto_download_interval_hours ?? 24}
									onchange={(e) => { if (config) { const v = parseInt((e.target as HTMLInputElement).value); config = { ...config, auto_download_interval_hours: v > 0 ? v : config.auto_download_interval_hours }; queueSave(); } }}
								/>
							</div>
							<div style="font-size: 0.8rem; color: var(--text2);">Changes to auto-download settings require a server restart.</div>
						</div>
				</div>
				</div>
				<div class="settings-col">
					<!-- Server -->
					<div class="panel">
						<div class="panel-title">
							Server
						</div>
						<div style="padding: 16px;">
							<label class="field-label" for="settings-port">Port</label>
							<input
								id="settings-port"
								type="number"
								class="input"
								style="max-width: 120px;"
								value={config.port}
								onchange={(e) => { if (config) { config = { ...config, port: parseInt((e.target as HTMLInputElement).value) || config.port }; queueSave(); } }}
							/>
						</div>
				</div>

					<!-- Features -->
					<div class="panel">
						<div class="panel-title">
							Features
						</div>
						<div style="padding: 16px; display: flex; flex-direction: column; gap: 14px;">
							<label style="display: flex; align-items: center; gap: 10px; cursor: pointer;">
								<input type="checkbox" checked={config.collections_enabled ?? true}
									onchange={() => { if (config) { config = { ...config, collections_enabled: !(config.collections_enabled ?? true) }; queueSave(); } }}
									style="width: 16px; height: 16px; accent-color: var(--accent);" />
								<div>
									<div style="font-weight: 600;">Enable collections</div>
									<div style="font-size: 0.8rem; color: var(--text2);">Track owned cards across named collections</div>
								</div>
							</label>
							<label style="display: flex; align-items: center; gap: 10px; cursor: pointer;">
								<input type="checkbox" checked={config.pricing_enabled ?? true}
									onchange={() => { if (config) { config = { ...config, pricing_enabled: !(config.pricing_enabled ?? true) }; queueSave(); } }}
									style="width: 16px; height: 16px; accent-color: var(--accent);" />
								<div>
									<div style="font-weight: 600;">Enable pricing</div>
									<div style="font-size: 0.8rem; color: var(--text2);">Show market prices and purchase history</div>
								</div>
							</label>
						</div>
				</div>

					<!-- File paths -->
					<div class="panel">
						<div class="panel-title">
							File Paths
						</div>
						<div style="padding: 16px; display: flex; flex-direction: column; gap: 12px;">
							{#each PATH_FIELDS as { key, label }}
								<div>
									<label class="field-label" for="settings-path-{key}">{label}</label>
									<input
										id="settings-path-{key}"
										type="text"
										class="input mono"
										value={(config as Record<string, unknown>)[key] as string ?? ''}
										oninput={(e) => setPath(key, (e.target as HTMLInputElement).value)}
										onchange={flushSave}
										placeholder="(default)"
									/>
								</div>
							{/each}
						</div>
				</div>
				</div>
			</div>
		{/if}
	</div>
</div>

<style>
	.settings-page {
		padding: 0 20px 40px;
		max-width: 1100px;
	}

	.save-status {
		font-size: 0.82rem;
		color: var(--text2);
	}

	/* Two independent columns (rather than grid rows) so a tall panel in one
	   doesn't stretch the other's. Collapses to one column on narrow screens. */
	.settings-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 20px;
		align-items: start;
	}

	.settings-col {
		display: flex;
		flex-direction: column;
		gap: 20px;
		min-width: 0;
	}

	@media (max-width: 860px) {
		.settings-grid { grid-template-columns: minmax(0, 1fr); }
	}

	.panel {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		overflow: hidden;
	}

	.panel-title {
		padding: 12px 16px;
		border-bottom: 1px solid var(--border);
		font-size: 0.78rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.07em;
		color: var(--text2);
	}

	.restart-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		flex-wrap: wrap;
		background: color-mix(in srgb, var(--accent) 15%, transparent);
		border: 1px solid var(--accent);
		border-radius: var(--radius);
		padding: 12px 16px;
		margin-bottom: 20px;
		font-size: 0.9rem;
	}
</style>
