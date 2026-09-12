<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { browser, dev } from '$app/environment';
	import DemoModeToast from '$lib/components/DemoModeToast.svelte';

	let { children } = $props();

	onMount(() => {
		if (browser && 'serviceWorker' in navigator) {
			navigator.serviceWorker.register('/service-worker.js', { type: dev ? 'module' : 'classic' });
		}
	});
</script>

<DemoModeToast />
{@render children()}
