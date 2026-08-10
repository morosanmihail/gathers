<script lang="ts">
	import { app } from '$lib/state.svelte';
	import { getRandomMtgCard, getRandomRiftboundCard, getRandomPokemonCard, addCardToCollection } from '$lib/api';
	import CardTile from '$lib/components/CardTile.svelte';
	import CardDetailModal from '$lib/components/CardDetailModal.svelte';
	import type { AnyCard, CollectionCard } from '$lib/types';

	let selectedSystems = $state<string[]>([]);
	let card = $state<AnyCard | null>(null);
	let loading = $state(false);
	let error = $state('');
	let detailCard = $state<AnyCard | null>(null);

	let addTarget = $state<AnyCard | null>(null);
	let addCollection = $state('');
	let toast = $state('');

	// Default to every enabled system once system info has loaded.
	$effect(() => {
		if (selectedSystems.length === 0 && app.systems.length > 0) {
			selectedSystems = [...app.systems];
		}
	});

	$effect(() => {
		if (app.collections.length > 0 && !addCollection) {
			addCollection = app.collections[0].id;
		}
	});

	function systemLabel(sys: string): string {
		return sys.replace('SQLite', '').replace('Sql', '');
	}

	function toggleSystem(sys: string) {
		selectedSystems = selectedSystems.includes(sys)
			? selectedSystems.filter(s => s !== sys)
			: [...selectedSystems, sys];
	}

	async function drawRandom() {
		if (selectedSystems.length === 0 || loading) return;
		loading = true;
		error = '';
		const pick = selectedSystems[Math.floor(Math.random() * selectedSystems.length)];
		try {
			card = pick === 'RiftboundSQLite'
				? await getRandomRiftboundCard()
				: pick === 'PokemonSQLite'
				? await getRandomPokemonCard()
				: await getRandomMtgCard();
		} catch (e) {
			error = String(e);
			card = null;
		} finally {
			loading = false;
		}
	}

	function promptAdd(c: AnyCard | CollectionCard) {
		if (!app.collectionsEnabled) return;
		addTarget = c as AnyCard;
	}

	async function confirmAdd() {
		if (!addTarget || !addCollection) return;
		try {
			await app.withOp(`Adding ${addTarget.name}`, () =>
				addCardToCollection(addCollection, addTarget!.id, 1, 0, null)
			);
			toast = `Added "${addTarget.name}" to ${addCollection}`;
			setTimeout(() => toast = '', 3000);
		} catch (e) {
			toast = `Error: ${e}`;
			setTimeout(() => toast = '', 3000);
		} finally {
			addTarget = null;
		}
	}
</script>

<svelte:head>
	<title>Random Card - gatheRs</title>
</svelte:head>

<div>
	{#if toast}
		<div class="toast-stack">
			<div class="toast success">{toast}</div>
		</div>
	{/if}

	<div class="page-header">
		<h1 class="page-title" style="font-size: 1.6rem;">🎲 Random Card</h1>
	</div>

	{#if app.systems.length === 0}
		<div class="empty-state" style="padding: 80px 20px;">
			<div class="empty-state-icon">🎲</div>
			<div class="empty-state-text">No card systems are enabled yet.</div>
		</div>
	{:else}
		<div style="padding: 0 20px 16px;">
			<div class="checkbox-group">
				{#each app.systems as sys}
					<label class="chip-checkbox" class:checked={selectedSystems.includes(sys)}>
						<input
							type="checkbox"
							checked={selectedSystems.includes(sys)}
							onchange={() => toggleSystem(sys)}
						/>
						{systemLabel(sys)}
					</label>
				{/each}
			</div>
		</div>

		<div style="padding: 0 20px 24px;">
			<button
				class="btn btn-accent"
				onclick={drawRandom}
				disabled={selectedSystems.length === 0 || loading}
			>
				{#if loading}
					<div class="spinner"></div> Drawing…
				{:else}
					🎲 Random
				{/if}
			</button>
		</div>

		{#if error}
			<div class="empty-state" style="padding: 40px 20px;">
				<div class="empty-state-text" style="color: var(--danger);">{error}</div>
			</div>
		{:else if !card}
			<div class="empty-state" style="padding: 80px 20px;">
				<div class="empty-state-icon">🃏</div>
				<div class="empty-state-text">Press Random to draw a card.</div>
			</div>
		{:else}
			<div style="padding: 0 20px; max-width: 220px;">
				<CardTile
					{card}
					onclick={(c) => detailCard = c as AnyCard}
					onAdd={app.collectionsEnabled ? promptAdd : undefined}
				/>
			</div>
		{/if}
	{/if}
</div>

{#if detailCard}
	<CardDetailModal card={detailCard} onclose={() => detailCard = null} />
{/if}

<!-- Add to collection dialog -->
{#if addTarget}
	<div class="confirm-overlay" role="dialog" aria-modal="true">
		<div class="confirm-box">
			<h4>Add to collection</h4>
			<p>Add <strong>{addTarget.name}</strong> to:</p>
			{#if app.collections.length > 0}
				<select class="input" bind:value={addCollection} style="margin-bottom: 16px;">
					{#each app.collections as col}
						<option value={col.id}>{col.id}</option>
					{/each}
				</select>
			{:else}
				<p style="color: var(--danger); font-size: 0.85rem;">No collections yet. Create one first.</p>
			{/if}
			<div class="confirm-actions">
				<button class="btn" onclick={() => addTarget = null}>Cancel</button>
				<button class="btn btn-accent" onclick={confirmAdd} disabled={!app.collections.length}>Add</button>
			</div>
		</div>
	</div>
{/if}
