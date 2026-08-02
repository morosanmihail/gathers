<script lang="ts">
	import type { AnyCard, CollectionCard, MtgCard, RiftboundCard, PokemonCard } from '$lib/types';
	import { cardImageUrl, rarityClass } from '$lib/types';
	import { cachedImageUrl, syncCachedImageUrl } from '$lib/imageCache';
	import { app } from '$lib/state.svelte';
	import MtgCardDetail from './MtgCardDetail.svelte';
	import RiftboundCardDetail from './RiftboundCardDetail.svelte';
	import PokemonCardDetail from './PokemonCardDetail.svelte';

	interface Props {
		card: AnyCard | CollectionCard;
		onclose: () => void;
	}

	let { card, onclose }: Props = $props();

	// Duck-type the system from whichever fields are present — cards from search
	// results and from a collection listing (which merges card detail + entry
	// fields) share the same shapes.
	const mtg = $derived(card as MtgCard & CollectionCard);
	const rift = $derived(card as RiftboundCard & CollectionCard);
	const poke = $derived(card as PokemonCard & CollectionCard);
	const isMtg = $derived('colorIdentity' in card);
	const isRift = $derived('domains' in card);
	const isPoke = $derived('energyTypes' in card && !isMtg);

	const setName = $derived(
		mtg.setName ||
		(card.setCode ? app.cardSets.find(s => s.code.toLowerCase() === card.setCode!.toLowerCase())?.name : '') ||
		(isPoke ? card.setCode : '') ||
		''
	);

	// Pokemon's `setCode` field actually holds the full set name (e.g. "Paradox
	// Rift") — the short form key printed in the card's corner (e.g. "PAR")
	// lives in `setShortCode` instead.
	const setDisplayCode = $derived(isPoke ? poke.setShortCode : card.setCode);

	const rawImgUrl = $derived(cardImageUrl(card as Parameters<typeof cardImageUrl>[0]));
	let imgUrl = $state('');
	$effect(() => {
		const raw = rawImgUrl;
		if (!raw) { imgUrl = ''; return; }
		const cached = syncCachedImageUrl(raw);
		imgUrl = cached || '';
		cachedImageUrl(raw).then(u => { if (imgUrl !== u) imgUrl = u; });
	});

	let imgExpanded = $state(false);
	function toggleImgExpanded() {
		imgExpanded = !imgExpanded;
	}
</script>

<div
	class="modal-overlay"
	onclick={(e) => e.target === e.currentTarget && onclose()}
	onkeydown={(e) => e.key === 'Escape' && (imgExpanded ? (imgExpanded = false) : onclose())}
	role="dialog"
	aria-modal="true"
	tabindex="-1"
>
	<div class="modal card-detail-modal">
		<div class="modal-header">
			<h3>{card.name}</h3>
			<button class="btn btn-ghost btn-icon" onclick={onclose} title="Close">✕</button>
		</div>

		<div class="modal-body card-detail-body">
			<div class="card-detail-art">
				{#if imgUrl}
					{#if imgExpanded}
						<button class="img-expand-backdrop" onclick={toggleImgExpanded} aria-label="Shrink image"></button>
					{/if}
					<button
						class="card-detail-img-btn"
						class:expanded={imgExpanded}
						onclick={toggleImgExpanded}
						title={imgExpanded ? 'Click to shrink' : 'Click to enlarge'}
						aria-label={imgExpanded ? 'Shrink card image' : 'Enlarge card image'}
					>
						<img src={imgUrl} alt={card.name} />
					</button>
				{:else}
					<div class="card-detail-art-placeholder">
						<div style="font-size: 2.5rem;">🃏</div>
						<div>No image available</div>
					</div>
				{/if}
			</div>

			<div class="card-detail-info">
				<!-- Common meta -->
				<div class="card-detail-row">
					<span class="card-detail-label">Set</span>
					<span>{setName || '—'} {setDisplayCode ? `(${setDisplayCode.toUpperCase()})` : ''}</span>
				</div>
				<div class="card-detail-row">
					<span class="card-detail-label">Collector #</span>
					<span>{card.collectorNumber ?? '—'}</span>
				</div>
				{#if card.rarity}
					<div class="card-detail-row">
						<span class="card-detail-label">Rarity</span>
						<span class={rarityClass(card.rarity)}>{card.rarity}</span>
					</div>
				{/if}
				{#if (card as CollectionCard).quantity != null}
					<div class="card-detail-row">
						<span class="card-detail-label">Owned</span>
						<span>
							{(card as CollectionCard).quantity} normal
							{#if (card as CollectionCard).foilQuantity}, {(card as CollectionCard).foilQuantity}✦ foil{/if}
						</span>
					</div>
				{/if}

				{#if isMtg}
					<MtgCardDetail card={mtg} />
				{:else if isRift}
					<RiftboundCardDetail card={rift} />
				{:else if isPoke}
					<PokemonCardDetail card={poke} />
				{/if}
			</div>
		</div>
	</div>
</div>

<style>
	.card-detail-modal { max-width: 820px; }

	.card-detail-body {
		display: grid;
		grid-template-columns: 240px 1fr;
		gap: 20px;
		align-items: start;
	}

	@media (max-width: 620px) {
		.card-detail-body { grid-template-columns: 1fr; }
	}

	.card-detail-art img {
		width: 100%;
		border-radius: var(--radius-lg);
		display: block;
	}

	.card-detail-img-btn {
		display: block;
		width: 100%;
		padding: 0;
		border: none;
		background: none;
		cursor: zoom-in;
	}

	.card-detail-img-btn img {
		border-radius: var(--radius-lg);
	}

	.card-detail-img-btn.expanded {
		position: fixed;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: auto;
		height: 92vh;
		max-width: 92vw;
		z-index: 310;
		cursor: zoom-out;
	}

	.card-detail-img-btn.expanded img {
		height: 100%;
		width: auto;
		max-width: 92vw;
		box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
	}

	.img-expand-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.7);
		border: none;
		padding: 0;
		z-index: 300;
		cursor: zoom-out;
	}

	.card-detail-art-placeholder {
		width: 100%;
		aspect-ratio: 5 / 7;
		border-radius: var(--radius-lg);
		background: var(--surface2);
		color: var(--text2);
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 8px;
		font-size: 0.82rem;
		text-align: center;
	}

	.card-detail-info {
		display: flex;
		flex-direction: column;
		gap: 10px;
		min-width: 0;
	}

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

</style>
