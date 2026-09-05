<script lang="ts">
	import '../app.css';
	import DemoModeToast from '$lib/components/DemoModeToast.svelte';

	let { children } = $props();
</script>

<DemoModeToast />
{@render children()}
