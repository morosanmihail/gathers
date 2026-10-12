<script lang="ts">
	import { getCsvLayouts, exportCollection, type CsvColumn, type CsvField, type CsvLayoutsInfo } from '$lib/api';

	interface Props {
		collection: string;
		onclose: () => void;
	}

	let { collection, onclose }: Props = $props();

	// The last layout used, remembered in this browser only.
	const STORAGE_KEY = 'gathers:csvExportLayout';

	let info = $state<CsvLayoutsInfo | null>(null);
	let columns = $state<CsvColumn[]>([]);
	let presetId = $state('');
	let error = $state('');
	let exporting = $state(false);

	const labelOf = (field: CsvField) => info?.fields.find(f => f.field === field)?.label ?? field;
	const unused = $derived(info ? info.fields.filter(f => !columns.some(c => c.field === f.field)) : []);
	const preset = $derived(info?.presets.find(p => p.id === presetId));

	// Plain copies: `info` is reactive state, whose proxies structuredClone can't copy.
	const copyColumns = (cols: CsvColumn[]): CsvColumn[] => cols.map(c => ({ field: c.field, header: c.header }));

	function samePreset(a: CsvColumn[], b: CsvColumn[]): boolean {
		return a.length === b.length && a.every((c, i) => c.field === b[i].field && c.header === b[i].header);
	}

	// Shows which preset the columns match, if any, after every edit.
	$effect(() => {
		presetId = info?.presets.find(p => samePreset(p.layout.columns, columns))?.id ?? '';
	});

	async function load() {
		try {
			info = await getCsvLayouts();
			let saved: CsvColumn[] | null = null;
			try {
				const raw = localStorage.getItem(STORAGE_KEY);
				if (raw) saved = JSON.parse(raw);
			} catch { /* storage unavailable */ }
			const known = new Set(info.fields.map(f => f.field));
			columns = saved?.length && saved.every(c => known.has(c.field) && typeof c.header === 'string')
				? saved
				: copyColumns(info.presets[0].layout.columns);
		} catch (e) {
			error = String(e);
		}
	}
	load();

	function applyPreset(id: string) {
		const p = info?.presets.find(p => p.id === id);
		if (p) columns = copyColumns(p.layout.columns);
	}

	function move(i: number, delta: number) {
		const j = i + delta;
		if (j < 0 || j >= columns.length) return;
		const next = [...columns];
		[next[i], next[j]] = [next[j], next[i]];
		columns = next;
	}

	function addColumn(field: string) {
		const f = info?.fields.find(f => f.field === field);
		if (f) columns = [...columns, { field: f.field, header: f.default_header }];
	}

	function changeField(i: number, field: CsvField) {
		columns = columns.map((c, j) => (j === i ? { ...c, field } : c));
	}

	const problem = $derived.by(() => {
		if (columns.length === 0) return 'Add at least one column.';
		const headers = columns.map(c => c.header.trim());
		if (headers.some(h => !h)) return 'Every column needs a header.';
		const dup = headers.find((h, i) => headers.indexOf(h) !== i);
		if (dup) return `Two columns are both called "${dup}".`;
		if (!columns.some(c => ['quantity', 'foil_quantity', 'want_quantity'].includes(c.field)))
			return 'Without a quantity column, the file only lists which cards are in the collection.';
		return '';
	});
	const blocking = $derived(problem !== '' && !problem.startsWith('Without'));

	async function handleExport() {
		error = '';
		exporting = true;
		try {
			const layout = { columns: columns.map(c => ({ ...c, header: c.header.trim() })) };
			const blob = await exportCollection(collection, layout);
			try { localStorage.setItem(STORAGE_KEY, JSON.stringify(layout.columns)); } catch { /* storage unavailable */ }
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = `${collection}${preset && preset.id !== 'gathers' ? `-${preset.id}` : ''}.csv`;
			a.click();
			setTimeout(() => URL.revokeObjectURL(url), 1000);
			onclose();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			exporting = false;
		}
	}
</script>

<div class="modal-overlay" onclick={(e) => e.target === e.currentTarget && onclose()} onkeydown={(e) => e.key === 'Escape' && onclose()} role="dialog" aria-modal="true" tabindex="-1">
	<div class="modal" style="max-width: 640px;">
		<div class="modal-header">
			<h3>Export — {collection}</h3>
			<button class="btn btn-ghost btn-icon" onclick={onclose} aria-label="Close">✕</button>
		</div>

		<div class="modal-body">
			{#if !info && !error}
				<div class="loading-row"><div class="spinner"></div> Loading…</div>
			{:else if info}
				<label class="field-label" for="csv-export-preset">Format</label>
				<select id="csv-export-preset" class="input" value={presetId} onchange={(e) => applyPreset((e.target as HTMLSelectElement).value)}>
					{#each info.presets as p (p.id)}
						<option value={p.id}>{p.label}</option>
					{/each}
					<option value="" disabled>Custom</option>
				</select>
				<p class="hint">{preset?.description ?? 'Your own choice of columns.'}</p>

				<div class="columns">
					<div class="column-head">
						<span>Field</span><span>Header in file</span><span></span>
					</div>
					{#each columns as column, i (column.field)}
						{@const fieldInfo = info.fields.find(f => f.field === column.field)}
						<div class="column-row">
							<select class="input" value={column.field} title={fieldInfo?.description}
								onchange={(e) => changeField(i, (e.target as HTMLSelectElement).value as CsvField)}>
								<option value={column.field}>{labelOf(column.field)}</option>
								{#each unused as f (f.field)}
									<option value={f.field}>{f.label}</option>
								{/each}
							</select>
							<input class="input mono" bind:value={column.header} aria-label="Header for {labelOf(column.field)}" />
							<div class="row-actions">
								<button class="btn btn-ghost btn-sm" onclick={() => move(i, -1)} disabled={i === 0} aria-label="Move up">↑</button>
								<button class="btn btn-ghost btn-sm" onclick={() => move(i, 1)} disabled={i === columns.length - 1} aria-label="Move down">↓</button>
								<button class="btn btn-ghost btn-sm" onclick={() => (columns = columns.filter((_, j) => j !== i))} aria-label="Remove">✕</button>
							</div>
						</div>
					{/each}
				</div>

				{#if unused.length > 0}
					<select class="input add-column" value="" onchange={(e) => { const el = e.target as HTMLSelectElement; addColumn(el.value); el.value = ''; }}>
						<option value="" disabled>+ Add column…</option>
						{#each unused as f (f.field)}
							<option value={f.field}>{f.label}</option>
						{/each}
					</select>
				{/if}

				<div class="preview mono" title="First line of the file">{columns.map(c => c.header.trim()).join(',')}</div>

				{#if problem}
					<p class="hint" style:color={blocking ? 'var(--danger)' : 'var(--text2)'}>{problem}</p>
				{/if}
			{/if}

			{#if error}
				<p class="hint" style="color: var(--danger);">{error}</p>
			{/if}

			<div class="actions">
				<button class="btn" onclick={onclose}>Cancel</button>
				<button class="btn btn-accent" onclick={handleExport} disabled={!info || blocking || exporting}>
					{exporting ? 'Exporting…' : 'Download CSV'}
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

	.columns {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.column-head,
	.column-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto;
		gap: 8px;
		align-items: center;
	}

	.column-head {
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text2);
	}

	.column-head span:last-child { width: 96px; }

	.row-actions {
		display: flex;
		gap: 2px;
	}

	.add-column {
		margin-top: 8px;
		max-width: 220px;
	}

	.preview {
		margin-top: 14px;
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		background: var(--surface2);
		font-size: 0.78rem;
		overflow-x: auto;
		white-space: nowrap;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		margin-top: 16px;
	}

	@media (max-width: 520px) {
		.column-head { display: none; }
		.column-row { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
		.row-actions { grid-column: 1 / -1; justify-content: flex-end; }
	}
</style>
