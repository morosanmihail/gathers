<script lang="ts">
	interface Props {
		checked: boolean;
		ontoggle: (e: Event) => void;
		style?: string;
	}

	let { checked, ontoggle, style = '' }: Props = $props();

	function onclick(e: MouseEvent) { e.stopPropagation(); ontoggle(e); }
	function onkeydown(e: KeyboardEvent) { if (e.key === 'Enter') { e.stopPropagation(); ontoggle(e); } }
</script>

<div
	class="card-tile-select"
	class:checked
	{style}
	role="checkbox"
	aria-checked={checked}
	tabindex="0"
	{onclick}
	{onkeydown}
>
	{#if checked}
		<svg width="10" height="10" viewBox="0 0 10 10" fill="white">
			<path d="M1.5 5L4 7.5 8.5 2" stroke="white" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
		</svg>
	{/if}
</div>
