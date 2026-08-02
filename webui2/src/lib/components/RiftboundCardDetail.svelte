<script lang="ts">
	import type { RiftboundCard, CollectionCard } from '$lib/types';

	interface Props {
		card: RiftboundCard & CollectionCard;
	}

	let { card: rift }: Props = $props();
</script>

<div class="card-detail-row">
	<span class="card-detail-label">Artist(s)</span>
	<span>{rift.artists?.join(', ') || '—'}</span>
</div>
{#if rift.domains?.length}
	<div class="card-detail-row">
		<span class="card-detail-label">Domains</span>
		<span class="card-detail-chips">
			{#each rift.domains as d}<span class="chip-checkbox checked">{d}</span>{/each}
		</span>
	</div>
{/if}
{#if rift.text}
	<div class="card-detail-block">
		<span class="card-detail-label">Rules text</span>
		<p class="card-detail-text">{rift.text}</p>
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

	.card-detail-chips {
		display: flex;
		flex-wrap: wrap;
		gap: 5px;
	}
</style>
