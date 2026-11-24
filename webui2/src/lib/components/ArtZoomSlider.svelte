<script lang="ts">
	// Art view card size, right-aligned in whichever toolbar shows it. The
	// icons step the size; double-clicking the slider resets it.
	import {
		app, setArtZoom, ART_ZOOM_MIN, ART_ZOOM_MAX, ART_ZOOM_STEP, ART_ZOOM_DEFAULT
	} from '$lib/state.svelte';

	const BUTTON_STEP = ART_ZOOM_STEP * 3;
</script>

<div class="art-zoom" role="group" aria-label="Card size">
	<button
		class="art-zoom-btn"
		title="Smaller cards"
		aria-label="Smaller cards"
		disabled={app.artZoom <= ART_ZOOM_MIN}
		onclick={() => setArtZoom(app.artZoom - BUTTON_STEP)}
	>
		<svg width="10" height="12" viewBox="0 0 10 14" fill="currentColor" aria-hidden="true">
			<rect x="1" y="2" width="8" height="11" rx="1.2"/>
		</svg>
	</button>
	<input
		type="range"
		class="art-zoom-range"
		min={ART_ZOOM_MIN}
		max={ART_ZOOM_MAX}
		step={ART_ZOOM_STEP}
		value={app.artZoom}
		oninput={(e) => setArtZoom(Number(e.currentTarget.value))}
		ondblclick={() => setArtZoom(ART_ZOOM_DEFAULT)}
		title="Card size: {app.artZoom}px (double-click to reset)"
		aria-label="Card size"
		aria-valuetext="{app.artZoom} pixels wide"
	/>
	<button
		class="art-zoom-btn"
		title="Larger cards"
		aria-label="Larger cards"
		disabled={app.artZoom >= ART_ZOOM_MAX}
		onclick={() => setArtZoom(app.artZoom + BUTTON_STEP)}
	>
		<svg width="14" height="18" viewBox="0 0 10 14" fill="currentColor" aria-hidden="true">
			<rect x="0" y="0" width="10" height="14" rx="1.4"/>
		</svg>
	</button>
</div>

<style>
	.art-zoom {
		margin-left: auto;
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.art-zoom-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 26px;
		height: 26px;
		padding: 0;
		border: none;
		border-radius: var(--radius, 6px);
		background: none;
		color: var(--text2);
		cursor: pointer;
	}

	.art-zoom-btn:hover:not(:disabled) { color: var(--text); background: var(--surface2); }
	.art-zoom-btn:disabled { opacity: 0.35; cursor: default; }

	.art-zoom-range {
		width: 140px;
		accent-color: var(--accent);
		cursor: pointer;
	}

	@media (max-width: 640px) {
		.art-zoom-range { width: 100px; }
	}
</style>
