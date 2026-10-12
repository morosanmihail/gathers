<script lang="ts">
	import { getCsvLayouts, importCards, type CsvField, type CsvLayoutsInfo, type CsvPreset } from '$lib/api';
	import { parseCsvRows } from '$lib/csv';
	import { app } from '$lib/state.svelte';

	interface Props {
		collection: string;
		file: File;
		onclose: () => void;
		/** Called after a successful import. */
		ondone: () => void;
	}

	let { collection, file, onclose, ondone }: Props = $props();

	let info = $state<CsvLayoutsInfo | null>(null);
	let headers = $state<string[]>([]);
	let sample = $state<string[]>([]);
	/** Which of the file's columns each field is read from ('' = none). */
	let mapping = $state<Partial<Record<CsvField, string>>>({});
	let presetId = $state('');
	let error = $state('');
	let importing = $state(false);

	const importable = $derived(info?.fields.filter(f => f.import_use !== 'ignored') ?? []);
	const preset = $derived(info?.presets.find(p => p.id === presetId));

	/** The preset's mapping for this file: its columns that the file has. */
	function presetMapping(p: CsvPreset): Partial<Record<CsvField, string>> {
		const m: Partial<Record<CsvField, string>> = {};
		for (const c of p.layout.columns) {
			if (headers.includes(c.header)) m[c.field] = c.header;
		}
		return m;
	}

	function applyPreset(id: string) {
		presetId = id;
		const p = info?.presets.find(p => p.id === id);
		if (!p || !info) return;
		const m = presetMapping(p);
		// Fill what the preset leaves out from columns named like the field.
		const taken = new Set(Object.values(m));
		for (const f of importable) {
			if (m[f.field]) continue;
			const match = headers.find(h => !taken.has(h)
				&& [f.default_header, f.label].some(n => n.toLowerCase() === h.toLowerCase()));
			if (match) { m[f.field] = match; taken.add(match); }
		}
		mapping = m;
	}

	async function load() {
		try {
			const [layouts, text] = await Promise.all([getCsvLayouts(), file.text()]);
			info = layouts;
			const rows = parseCsvRows(text, 2);
			if (rows.length === 0) { error = 'The file is empty.'; return; }
			headers = rows[0].filter(h => h !== '');
			sample = rows[1] ?? [];
			// Start from the preset matching the most of the file's columns.
			let best = layouts.presets[0];
			let bestScore = -1;
			for (const p of layouts.presets) {
				const score = Object.keys(presetMapping(p)).length;
				if (score > bestScore) { best = p; bestScore = score; }
			}
			applyPreset(best.id);
		} catch (e) {
			error = String(e);
		}
	}
	load();

	function setMapping(field: CsvField, header: string) {
		mapping = { ...mapping, [field]: header };
		presetId = '';
	}

	function sampleOf(header: string | undefined): string {
		if (!header) return '';
		const i = headers.indexOf(header);
		return i >= 0 ? (sample[i] ?? '') : '';
	}

	const problem = $derived.by(() => {
		if (!info) return '';
		const has = (f: CsvField) => !!mapping[f];
		const header = (f: CsvField) => info!.fields.find(x => x.field === f)?.label ?? f;
		if (!has('id') && !(has('set_code') && has('collector_number')))
			return `Choose columns for ${header('set_code')} and ${header('collector_number')}, or for ${header('id')}, so cards can be found.`;
		if (!has('quantity') && !has('foil_quantity') && !has('want_quantity'))
			return 'Choose at least one quantity column.';
		return '';
	});

	async function handleImport() {
		error = '';
		importing = true;
		try {
			const columns = importable
				.filter(f => mapping[f.field])
				.map(f => ({ field: f.field, header: mapping[f.field]! }));
			await app.withOp('Importing', () => importCards(collection, file, { columns }));
			ondone();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			importing = false;
		}
	}
</script>

<div class="modal-overlay" onclick={(e) => e.target === e.currentTarget && !importing && onclose()} onkeydown={(e) => e.key === 'Escape' && !importing && onclose()} role="dialog" aria-modal="true" tabindex="-1">
	<div class="modal" style="max-width: 640px;">
		<div class="modal-header">
			<h3>Import into {collection}</h3>
			<button class="btn btn-ghost btn-icon" onclick={onclose} disabled={importing} aria-label="Close">✕</button>
		</div>

		<div class="modal-body">
			<p class="hint" style="margin-top: 0;">
				<span class="mono">{file.name}</span>: pick which of its columns holds each value. Other columns are ignored.
			</p>

			{#if !info && !error}
				<div class="loading-row"><div class="spinner"></div> Reading file…</div>
			{:else if info && headers.length > 0}
				<label class="field-label" for="csv-import-preset">Format</label>
				<select id="csv-import-preset" class="input" value={presetId} onchange={(e) => applyPreset((e.target as HTMLSelectElement).value)}>
					{#each info.presets as p (p.id)}
						<option value={p.id}>{p.label}</option>
					{/each}
					<option value="" disabled>Custom</option>
				</select>
				<p class="hint">{preset?.description ?? 'Your own choice of columns.'}</p>

				<div class="mapping">
					<div class="mapping-head">
						<span>Value</span><span>Column in file</span><span>First row</span>
					</div>
					{#each importable as f (f.field)}
						<div class="mapping-row">
							<label for="csv-map-{f.field}" title={f.description}>
								{f.label}
								{#if f.import_use === 'identifies'}<span class="tag">finds card</span>{/if}
							</label>
							<select id="csv-map-{f.field}" class="input" value={mapping[f.field] ?? ''}
								onchange={(e) => setMapping(f.field, (e.target as HTMLSelectElement).value)}>
								<option value="">— not in file —</option>
								{#each headers as h (h)}
									<option value={h}>{h}</option>
								{/each}
							</select>
							<span class="sample mono" title={sampleOf(mapping[f.field])}>{sampleOf(mapping[f.field])}</span>
						</div>
					{/each}
				</div>

				<p class="hint">
					Cards are found by id when given, otherwise by set and collector number. Values without a column default to empty or 0.
				</p>
				{#if problem}
					<p class="hint" style="color: var(--danger);">{problem}</p>
				{/if}
			{/if}

			{#if error}
				<p class="hint" style="color: var(--danger);">{error}</p>
			{/if}

			<div class="actions">
				<button class="btn" onclick={onclose} disabled={importing}>Cancel</button>
				<button class="btn btn-accent" onclick={handleImport} disabled={!info || headers.length === 0 || !!problem || importing}>
					{importing ? 'Importing…' : 'Import'}
				</button>
			</div>
		</div>
	</div>
</div>

<style>
	.hint {
		margin: 6px 0 14px;
		font-size: 0.8rem;
		color: var(--text2);
	}

	.mapping {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.mapping-head,
	.mapping-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr) minmax(0, 0.8fr);
		gap: 8px;
		align-items: center;
	}

	.mapping-head {
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text2);
	}

	.mapping-row label {
		font-size: 0.85rem;
		font-weight: 600;
	}

	.tag {
		margin-left: 4px;
		font-size: 0.68rem;
		font-weight: 500;
		color: var(--text2);
	}

	.sample {
		font-size: 0.78rem;
		color: var(--text2);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		margin-top: 16px;
	}

	@media (max-width: 520px) {
		.mapping-head { display: none; }
		.mapping-row { grid-template-columns: minmax(0, 1fr) minmax(0, 1.4fr); }
		.sample { display: none; }
	}
</style>
