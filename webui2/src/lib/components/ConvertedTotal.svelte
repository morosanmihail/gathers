<script lang="ts">
	import { convertTotal, convertTotalsEnabled, formatMoney, getRates, preferredCurrency, type Money } from '$lib/currency.svelte';

	interface Props {
		items: Money[];
	}

	let { items }: Props = $props();

	let rates = $state<Record<string, number> | null>(null);

	$effect(() => {
		if (convertTotalsEnabled() && !rates) getRates().then(r => { rates = r; });
	});

	const target = $derived(preferredCurrency());
	// Only worth showing when something actually needs converting.
	const needed = $derived(items.some(m => m.currency !== target && m.value !== 0));
	const total = $derived(rates ? convertTotal(items, target, rates) : null);
</script>

{#if convertTotalsEnabled() && needed && total != null}
	<span class="converted-total" title="Converted to {target} at current ECB reference rates">= {formatMoney(total, target)}</span>
{/if}
