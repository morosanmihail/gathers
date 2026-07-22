<script lang="ts">
	import { getAllPurchaseHistory, deletePurchaseEntry, updatePurchaseEntry, type PurchaseEntry } from '$lib/api';
	import ConfirmDialog from './ConfirmDialog.svelte';
	import { portal } from '$lib/portal';
	import { fmtDate } from '$lib/format';

	interface Props {
		collection: string;
		onclose: () => void;
	}

	let { collection, onclose }: Props = $props();

	const PAGE_SIZE = 25;

	let entries = $state<PurchaseEntry[]>([]);
	let loading = $state(true);
	let error = $state('');
	let currentPage = $state(1);

	const totalPages = $derived(Math.max(1, Math.ceil(entries.length / PAGE_SIZE)));
	const pagedEntries = $derived(entries.slice((currentPage - 1) * PAGE_SIZE, currentPage * PAGE_SIZE));

	// Per-row edit state keyed by entry id
	interface EditState {
		quantity: string;
		foil_quantity: string;
		normal_price: string;
		foil_price: string;
		saving: boolean;
	}
	let editing = $state<Map<number, EditState>>(new Map());
	let confirmDeleteId = $state<number | null>(null);

	async function load(resetPage = false) {
		loading = true;
		error = '';
		try {
			entries = await getAllPurchaseHistory(collection);
			if (resetPage) currentPage = 1;
			else currentPage = Math.min(currentPage, Math.max(1, Math.ceil(entries.length / PAGE_SIZE)));
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	load();

	function startEdit(e: PurchaseEntry) {
		const m = new Map(editing);
		m.set(e.id, {
			quantity: String(e.quantity),
			foil_quantity: String(e.foil_quantity),
			normal_price: e.normal_price_per_unit != null ? String(e.normal_price_per_unit) : '',
			foil_price: e.foil_price_per_unit != null ? String(e.foil_price_per_unit) : '',
			saving: false
		});
		editing = m;
	}

	function cancelEdit(id: number) {
		const m = new Map(editing);
		m.delete(id);
		editing = m;
	}

	async function saveEdit(id: number) {
		const s = editing.get(id);
		if (!s) return;
		const m = new Map(editing);
		m.set(id, { ...s, saving: true });
		editing = m;
		try {
			await updatePurchaseEntry(
				collection, id,
				parseInt(s.quantity) || 0,
				parseInt(s.foil_quantity) || 0,
				s.normal_price !== '' ? parseFloat(s.normal_price) : null,
				s.foil_price !== '' ? parseFloat(s.foil_price) : null
			);
			await load();
		} catch (e) {
			error = String(e);
		} finally {
			const m2 = new Map(editing);
			m2.delete(id);
			editing = m2;
		}
	}

	async function doDelete(id: number) {
		confirmDeleteId = null;
		try {
			await deletePurchaseEntry(collection, id);
			await load();
		} catch (e) {
			error = String(e);
		}
	}


</script>

<div class="modal-overlay" onclick={(e) => e.target === e.currentTarget && onclose()} onkeydown={(e) => e.key === 'Escape' && onclose()} role="dialog" aria-modal="true" tabindex="-1">
	<div class="modal" style="max-width: 860px;">
		<div class="modal-header">
			<h3>Purchase History — {collection}</h3>
			<button class="btn btn-ghost btn-icon" onclick={onclose}>✕</button>
		</div>

		<div class="modal-body" style="padding: 0; overflow-x: auto;">
			{#if error}
				<div style="padding: 16px; color: var(--danger);">{error}</div>
			{/if}

			{#if loading}
				<div class="loading-row"><div class="spinner"></div> Loading…</div>
			{:else if entries.length === 0}
				<div class="empty-state" style="padding: 48px;">
					<div class="empty-state-icon">📋</div>
					<div class="empty-state-text">No purchase history recorded</div>
				</div>
			{:else}
				<table style="width:100%; border-collapse: collapse; font-size: 0.85rem;">
					<thead>
						<tr style="background: var(--bg2); border-bottom: 2px solid var(--border);">
							<th style="padding: 10px 14px; text-align:left; font-size:0.72rem; text-transform:uppercase; letter-spacing:0.07em; color:var(--text2); white-space:nowrap;">Card</th>
							<th style="padding: 10px 14px; text-align:left; font-size:0.72rem; text-transform:uppercase; letter-spacing:0.07em; color:var(--text2);">Set</th>
							<th style="padding: 10px 14px; text-align:left; font-size:0.72rem; text-transform:uppercase; letter-spacing:0.07em; color:var(--text2);">Date</th>
							<th style="padding: 10px 14px; text-align:center; font-size:0.72rem; text-transform:uppercase; letter-spacing:0.07em; color:var(--text2);">Qty</th>
							<th style="padding: 10px 14px; text-align:center; font-size:0.72rem; text-transform:uppercase; letter-spacing:0.07em; color:var(--text2);">Foil</th>
							<th style="padding: 10px 14px; text-align:right; font-size:0.72rem; text-transform:uppercase; letter-spacing:0.07em; color:var(--text2);">Normal price</th>
							<th style="padding: 10px 14px; text-align:right; font-size:0.72rem; text-transform:uppercase; letter-spacing:0.07em; color:var(--text2);">Foil price</th>
							<th style="padding: 10px 14px;"></th>
						</tr>
					</thead>
					<tbody>
						{#each pagedEntries as entry (entry.id)}
							{@const ed = editing.get(entry.id)}
							<tr style="border-bottom: 1px solid var(--border); transition: background 0.1s;" class:editing={!!ed}>
								<td style="padding: 8px 14px; color: var(--text); font-weight: 600;">
									{entry.card_name ?? entry.card_uuid}
								</td>
								<td style="padding: 8px 14px; color: var(--text2); font-family: 'JetBrains Mono', monospace; font-size: 0.78rem;">
									{entry.set_code ?? '—'}
								</td>
								<td style="padding: 8px 14px; color: var(--text2); white-space: nowrap;">
									{fmtDate(entry.recorded_at)}
								</td>

								{#if ed}
									<td style="padding: 4px 8px;">
										<input class="input" style="width:60px;height:28px;padding:3px 6px;text-align:center;"
											bind:value={ed.quantity} type="number" min="0" />
									</td>
									<td style="padding: 4px 8px;">
										<input class="input" style="width:60px;height:28px;padding:3px 6px;text-align:center;"
											bind:value={ed.foil_quantity} type="number" min="0" />
									</td>
									<td style="padding: 4px 8px;">
										<input class="input" style="width:80px;height:28px;padding:3px 6px;text-align:right;font-family:'JetBrains Mono',monospace;"
											bind:value={ed.normal_price} placeholder="—" />
									</td>
									<td style="padding: 4px 8px;">
										<input class="input" style="width:80px;height:28px;padding:3px 6px;text-align:right;font-family:'JetBrains Mono',monospace;"
											bind:value={ed.foil_price} placeholder="—" />
									</td>
									<td style="padding: 4px 8px; white-space: nowrap; display: flex; gap: 4px;">
										<button class="btn btn-sm btn-accent" disabled={ed.saving} onclick={() => saveEdit(entry.id)}>
											{ed.saving ? '…' : 'Save'}
										</button>
										<button class="btn btn-sm btn-ghost" onclick={() => cancelEdit(entry.id)}>Cancel</button>
									</td>
								{:else}
									<td style="padding: 8px 14px; text-align:center; font-family:'JetBrains Mono',monospace;">
										{entry.quantity}
									</td>
									<td style="padding: 8px 14px; text-align:center; font-family:'JetBrains Mono',monospace; color: var(--accent-text);">
										{entry.foil_quantity}✦
									</td>
									<td style="padding: 8px 14px; text-align:right; font-family:'JetBrains Mono',monospace; color: var(--accent-text);">
										{entry.normal_price_per_unit != null ? `$${entry.normal_price_per_unit.toFixed(2)}` : '—'}
									</td>
									<td style="padding: 8px 14px; text-align:right; font-family:'JetBrains Mono',monospace; color: var(--accent-text);">
										{entry.foil_price_per_unit != null ? `$${entry.foil_price_per_unit.toFixed(2)}` : '—'}
									</td>
									<td style="padding: 8px 14px; white-space: nowrap;">
										<button class="btn btn-sm" onclick={() => startEdit(entry)}>Edit</button>
										<button class="btn btn-sm btn-danger" style="margin-left:4px;" onclick={() => confirmDeleteId = entry.id}>Delete</button>
									</td>
								{/if}
							</tr>
						{/each}
					</tbody>
				</table>
				{#if totalPages > 1}
					<div style="display:flex; align-items:center; justify-content:center; gap:12px; padding:12px 14px; border-top:1px solid var(--border);">
						<button class="btn btn-sm btn-ghost" disabled={currentPage === 1} onclick={() => currentPage--}>‹ Prev</button>
						<span style="font-size:0.82rem; color:var(--text2);">
							{currentPage} / {totalPages}
							<span style="margin-left:6px; color:var(--text2);">({entries.length} entries)</span>
						</span>
						<button class="btn btn-sm btn-ghost" disabled={currentPage === totalPages} onclick={() => currentPage++}>Next ›</button>
					</div>
				{/if}
			{/if}
		</div>
	</div>
</div>

{#if confirmDeleteId !== null}
	<ConfirmDialog
		title="Delete entry"
		message="Remove this purchase history entry? This cannot be undone."
		confirmLabel="Delete"
		danger
		onconfirm={() => doDelete(confirmDeleteId!)}
		oncancel={() => confirmDeleteId = null}
	/>
{/if}

<style>
	tr.editing { background: var(--surface2); }
	tr:hover:not(.editing) { background: var(--surface); }
</style>
