<script lang="ts">
	import { PAGE_SIZE } from '$lib/api';

	interface Props {
		total: number;
		page: number;
		onchange: (p: number) => void;
		pageSize?: number;
	}

	let { total, page, onchange, pageSize = PAGE_SIZE }: Props = $props();

	const totalPages = $derived(Math.max(1, Math.ceil(total / pageSize)));

	function pages(): Array<number | '…'> {
		if (totalPages <= 7) return Array.from({ length: totalPages }, (_, i) => i + 1);
		const result: Array<number | '…'> = [1];
		if (page > 3) result.push('…');
		for (let i = Math.max(2, page - 1); i <= Math.min(totalPages - 1, page + 1); i++) result.push(i);
		if (page < totalPages - 2) result.push('…');
		result.push(totalPages);
		return result;
	}
</script>

{#if totalPages > 1}
	<div class="pagination">
		<button class="page-btn" disabled={page <= 1} onclick={() => onchange(page - 1)}>‹</button>
		{#each pages() as p}
			{#if p === '…'}
				<span class="page-btn" style="cursor:default;border:none;">…</span>
			{:else}
				<button class="page-btn" class:active={p === page} onclick={() => onchange(p)}>{p}</button>
			{/if}
		{/each}
		<button class="page-btn" disabled={page >= totalPages} onclick={() => onchange(page + 1)}>›</button>
	</div>
{/if}
