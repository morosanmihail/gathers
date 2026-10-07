<script lang="ts">
	import { BASE_CURRENCY, currencyPrefs, formatMoney, getEurRates, toEurTotal, type Money } from '$lib/currency.svelte';

	interface Props {
		items: Money[];
	}

	let { items }: Props = $props();

	let rates = $state<Record<string, number> | null>(null);

	$effect(() => {
		if (currencyPrefs.convertToEur && !rates) getEurRates().then(r => { rates = r; });
	});

	// Only worth showing when something actually needs converting.
	const needed = $derived(items.some(m => m.currency !== BASE_CURRENCY && m.value !== 0));
	const total = $derived(rates ? toEurTotal(items, rates) : null);
</script>

{#if currencyPrefs.convertToEur && needed && total != null}
	<span class="eur-total" title="Converted to EUR at current ECB reference rates">≈ {formatMoney(total, BASE_CURRENCY)} total</span>
{/if}
