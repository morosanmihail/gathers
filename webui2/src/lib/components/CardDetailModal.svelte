<script lang="ts">
	import type { AnyCard, CollectionCard, MtgCard, RiftboundCard, PokemonCard } from '$lib/types';
	import { cardImageUrl, legalityFormats, rarityClass } from '$lib/types';
	import { cachedImageUrl, syncCachedImageUrl } from '$lib/imageCache';
	import { app } from '$lib/state.svelte';
	import { fmtDate } from '$lib/format';

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
		''
	);

	const rawImgUrl = $derived(cardImageUrl(card as Parameters<typeof cardImageUrl>[0]));
	let imgUrl = $state('');
	$effect(() => {
		const raw = rawImgUrl;
		if (!raw) { imgUrl = ''; return; }
		const cached = syncCachedImageUrl(raw);
		imgUrl = cached || '';
		cachedImageUrl(raw).then(u => { if (imgUrl !== u) imgUrl = u; });
	});

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

<div
	class="modal-overlay"
	onclick={(e) => e.target === e.currentTarget && onclose()}
	onkeydown={(e) => e.key === 'Escape' && onclose()}
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
					<img src={imgUrl} alt={card.name} />
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
					<span>{setName || '—'} {card.setCode ? `(${card.setCode.toUpperCase()})` : ''}</span>
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
				{:else if isRift}
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
				{:else if isPoke}
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
