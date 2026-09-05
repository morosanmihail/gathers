<script lang="ts">
	import { demoAlert } from '$lib/demoAlert.svelte';
</script>

{#if demoAlert.visible}
	<div class="demo-toast-stack">
		<div class="demo-toast" role="alert">
			<span class="demo-toast-icon">🔒</span>
			<div class="demo-toast-text">
				<strong>Demo Mode</strong>
				<span>This action is disabled in the public demo.</span>
			</div>
			<button class="demo-toast-close" aria-label="Dismiss" onclick={() => demoAlert.dismiss()}>&times;</button>
		</div>
	</div>
{/if}
