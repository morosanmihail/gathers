<script lang="ts">
	import type { PokemonCard, CollectionCard } from '$lib/types';
	import { fmtDate } from '$lib/format';

	interface Props {
		card: PokemonCard & CollectionCard;
	}

	let { card: poke }: Props = $props();
</script>

{#if poke.cardType}
	<div class="card-detail-row">
		<span class="card-detail-label">Card type</span>
		<span>{poke.cardType}</span>
	</div>
{/if}
{#if poke.energyTypes?.length}
	<div class="card-detail-row">
		<span class="card-detail-label">Energy</span>
		<span class="card-detail-chips">
			{#each poke.energyTypes as e}<span class="chip-checkbox checked">{e}</span>{/each}
		</span>
	</div>
{/if}
{#if poke.pokedex}
	<div class="card-detail-row">
		<span class="card-detail-label">National Pokédex #</span>
		<span>{poke.pokedex}</span>
	</div>
{/if}
{#if poke.releaseDate}
	<div class="card-detail-row">
		<span class="card-detail-label">Released</span>
		<span>{fmtDate(poke.releaseDate)}</span>
	</div>
{/if}
{#if poke.description}
	<div class="card-detail-block">
		<span class="card-detail-label">Description</span>
		<p class="card-detail-text">{poke.description}</p>
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
