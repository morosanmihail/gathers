<script lang="ts">
	import type { MtgCard, CollectionCard } from '$lib/types';
	import { legalityFormats } from '$lib/types';

	interface Props {
		card: MtgCard & CollectionCard;
	}

	let { card: mtg }: Props = $props();

	const COLOR_LETTER: Record<string, string> = {
		White: 'W', Blue: 'U', Black: 'B', Red: 'R', Green: 'G',
		Colourless: 'C', Multicoloured: 'M'
	};

	function legalityLabel(format: string): string {
		return legalityFormats.find(f => f.value === format.toLowerCase())?.label ?? format;
	}

	function legalityClass(status: string): string {
		const s = status.toLowerCase();
		if (s === 'legal') return 'legality-legal';
		if (s === 'banned') return 'legality-banned';
		if (s === 'restricted') return 'legality-restricted';
		return 'legality-other';
	}

	const ptLine = $derived.by(() => {
		if (mtg.power != null || mtg.toughness != null) return `${mtg.power ?? '*'} / ${mtg.toughness ?? '*'}`;
		if (mtg.loyalty != null) return `Loyalty: ${mtg.loyalty}`;
		if (mtg.defense != null) return `Defense: ${mtg.defense}`;
		return '';
	});

	const flags = $derived.by(() => {
		const out: string[] = [];
		if (mtg.isReserved) out.push('Reserved List');
		if (mtg.isPromo) out.push('Promo');
		if (mtg.isReprint) out.push('Reprint');
		if (mtg.isFullArt) out.push('Full Art');
		return out;
	});
</script>

<div class="card-detail-row">
	<span class="card-detail-label">Artist</span>
	<span>{mtg.artist || '—'}</span>
</div>
{#if mtg.typeLine}
	<div class="card-detail-row">
		<span class="card-detail-label">Type</span>
		<span>{mtg.typeLine}</span>
	</div>
{/if}
{#if mtg.manaCost || mtg.manaValue != null}
	<div class="card-detail-row">
		<span class="card-detail-label">Mana cost</span>
		<span class="mono">{mtg.manaCost || '—'} {mtg.manaValue != null ? `(MV ${mtg.manaValue})` : ''}</span>
	</div>
{/if}
{#if ptLine}
	<div class="card-detail-row">
		<span class="card-detail-label">P/T</span>
		<span class="mono">{ptLine}</span>
	</div>
{/if}

<div class="card-detail-row">
	<span class="card-detail-label">Color identity</span>
	<span class="card-detail-chips">
		{#each mtg.colorIdentity ?? [] as c}
			<span class="color-chip color-chip-sm active {COLOR_LETTER[c] ?? ''}" title={c}>{COLOR_LETTER[c] ?? c[0]}</span>
		{:else}—{/each}
	</span>
</div>
{#if mtg.colors?.length}
	<div class="card-detail-row">
		<span class="card-detail-label">Colors</span>
		<span class="card-detail-chips">
			{#each mtg.colors as c}
				<span class="color-chip color-chip-sm active {COLOR_LETTER[c] ?? ''}" title={c}>{COLOR_LETTER[c] ?? c[0]}</span>
			{/each}
		</span>
	</div>
{/if}
{#if mtg.keywords?.length}
	<div class="card-detail-row">
		<span class="card-detail-label">Keywords</span>
		<span class="card-detail-chips">
			{#each mtg.keywords as k}<span class="chip-checkbox checked">{k}</span>{/each}
		</span>
	</div>
{/if}

{#if mtg.text}
	<div class="card-detail-block">
		<span class="card-detail-label">Rules text</span>
		<p class="card-detail-text">{mtg.text}</p>
	</div>
{/if}
{#if mtg.flavorText}
	<div class="card-detail-block">
		<p class="card-detail-flavor">{mtg.flavorText}</p>
	</div>
{/if}

{#if flags.length}
	<div class="card-detail-row">
		<span class="card-detail-label">Flags</span>
		<span class="card-detail-chips">
			{#each flags as f}<span class="chip-checkbox checked">{f}</span>{/each}
		</span>
	</div>
{/if}
<div class="card-detail-row">
	<span class="card-detail-label">Border / Finish</span>
	<span>
		{mtg.borderColor ?? '—'}{mtg.finishes?.length ? ` · ${mtg.finishes.join(', ')}` : ''}
	</span>
</div>
{#if mtg.watermark}
	<div class="card-detail-row">
		<span class="card-detail-label">Watermark</span>
		<span>{mtg.watermark}</span>
	</div>
{/if}

{#if mtg.legalities && Object.keys(mtg.legalities).length}
	<div class="card-detail-block">
		<span class="card-detail-label">Legalities</span>
		<div class="legality-grid">
			{#each Object.entries(mtg.legalities) as [format, status]}
				<span class="legality-item {legalityClass(status)}">
					<span class="legality-format">{legalityLabel(format)}</span>
					<span class="legality-status">{status}</span>
				</span>
			{/each}
		</div>
	</div>
{/if}

<style>
	.card-detail-row {
		display: flex;
		gap: 8px;
		align-items: baseline;
		font-size: 0.88rem;
	}

	.card-detail-label {
		flex-shrink: 0;
		width: 110px;
		color: var(--text2);
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.card-detail-block { display: flex; flex-direction: column; gap: 4px; }

	.card-detail-text {
		white-space: pre-wrap;
		font-size: 0.88rem;
		line-height: 1.4;
	}

	.card-detail-flavor {
		white-space: pre-wrap;
		font-size: 0.85rem;
		font-style: italic;
		color: var(--text2);
	}

	.card-detail-chips {
		display: flex;
		flex-wrap: wrap;
		gap: 5px;
	}

	.color-chip-sm {
		width: 22px;
		height: 22px;
		font-size: 0.68rem;
	}

	.legality-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
		gap: 6px;
	}

	.legality-item {
		display: flex;
		justify-content: space-between;
		gap: 6px;
		padding: 3px 8px;
		border-radius: 6px;
		font-size: 0.76rem;
		background: var(--surface2);
		border: 1px solid var(--border2);
	}

	.legality-format { color: var(--text2); }
	.legality-status { font-weight: 700; }

	.legality-legal .legality-status { color: var(--success, #4caf82); }
	.legality-banned .legality-status { color: var(--danger); }
	.legality-restricted .legality-status { color: #d8a034; }
	.legality-other .legality-status { color: var(--text2); }
</style>
