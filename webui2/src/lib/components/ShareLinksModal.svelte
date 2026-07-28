<script lang="ts">
	import { listShareLinks, createShareLink, revokeShareLink, type ShareLink } from '$lib/api';
	import ConfirmDialog from './ConfirmDialog.svelte';
	import { fmtDate } from '$lib/format';

	interface Props {
		collection: string;
		onclose: () => void;
	}

	let { collection, onclose }: Props = $props();

	let links = $state<ShareLink[]>([]);
	let loading = $state(true);
	let error = $state('');
	let creating = $state(false);
	let confirmRevoke = $state<ShareLink | null>(null);
	let copiedToken = $state('');

	function shareUrl(token: string): string {
		return `${window.location.origin}/share/${encodeURIComponent(token)}`;
	}

	async function load() {
		loading = true;
		error = '';
		try {
			links = await listShareLinks(collection);
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	load();

	async function handleCreate() {
		creating = true;
		error = '';
		try {
			const link = await createShareLink(collection);
			links = [link, ...links];
		} catch (e) {
			error = String(e);
		} finally {
			creating = false;
		}
	}

	async function handleRevoke() {
		if (!confirmRevoke) return;
		const token = confirmRevoke.token;
		confirmRevoke = null;
		try {
			await revokeShareLink(collection, token);
			links = links.filter(l => l.token !== token);
		} catch (e) {
			error = String(e);
		}
	}

	async function copyLink(token: string) {
		try {
			await navigator.clipboard.writeText(shareUrl(token));
			copiedToken = token;
			setTimeout(() => { if (copiedToken === token) copiedToken = ''; }, 1500);
		} catch (e) {
			error = String(e);
		}
	}
</script>

<div class="modal-overlay" onclick={(e) => e.target === e.currentTarget && onclose()} onkeydown={(e) => e.key === 'Escape' && onclose()} role="dialog" aria-modal="true" tabindex="-1">
	<div class="modal" style="max-width: 640px;">
		<div class="modal-header">
			<h3>Share links — {collection}</h3>
			<button class="btn btn-ghost btn-icon" onclick={onclose}>✕</button>
		</div>

		<div class="modal-body">
			<p style="margin: 0 0 12px; color: var(--text2); font-size: 0.85rem;">
				Anyone with a link below can view this collection read-only, without needing an account. Revoke a link to invalidate it immediately.
			</p>

			{#if error}
				<div style="color: var(--danger); margin-bottom: 12px; font-size: 0.85rem;">{error}</div>
			{/if}

			<button class="btn btn-accent" disabled={creating} onclick={handleCreate} style="margin-bottom: 16px;">
				{creating ? 'Creating…' : '+ New share link'}
			</button>

			{#if loading}
				<div class="loading-row"><div class="spinner"></div> Loading…</div>
			{:else if links.length === 0}
				<div class="empty-state" style="padding: 32px;">
					<div class="empty-state-icon">🔗</div>
					<div class="empty-state-text">No share links yet.</div>
				</div>
			{:else}
				<div style="display: flex; flex-direction: column; gap: 8px;">
					{#each links as link (link.token)}
						<div style="display: flex; align-items: center; gap: 8px; padding: 8px 10px; border: 1px solid var(--border); border-radius: 8px;">
							<input
								class="input"
								readonly
								value={shareUrl(link.token)}
								style="flex: 1; font-family: 'JetBrains Mono', monospace; font-size: 0.78rem; height: 32px; padding: 4px 8px;"
								onclick={(e) => (e.target as HTMLInputElement).select()}
							/>
							<span style="color: var(--text2); font-size: 0.75rem; white-space: nowrap;">
								{fmtDate(link.createdAt)}
							</span>
							<button class="btn btn-sm" onclick={() => copyLink(link.token)}>
								{copiedToken === link.token ? 'Copied!' : 'Copy'}
							</button>
							<button class="btn btn-sm btn-danger" onclick={() => confirmRevoke = link}>
								Revoke
							</button>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</div>

{#if confirmRevoke}
	<ConfirmDialog
		title="Revoke share link"
		message="Anyone using this link will immediately lose access to '{collection}'. This cannot be undone."
		confirmLabel="Revoke"
		danger
		onconfirm={handleRevoke}
		oncancel={() => confirmRevoke = null}
	/>
{/if}
