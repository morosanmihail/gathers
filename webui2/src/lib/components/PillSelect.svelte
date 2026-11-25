<script lang="ts">
	// A compact multi-select: only the chosen values show, as removable pills.
	// Clicking the field opens the full list inline (not as a floating popover,
	// since the search sidebar and the search modal both scroll and would clip
	// one), with a text box to narrow it down.
	interface Props {
		options: string[];
		selected: string[];
		onchange: (selected: string[]) => void;
		/** Shown when nothing is selected, e.g. "Any type". */
		placeholder?: string;
		/** Accessible name for the field, e.g. "Card type". */
		label: string;
	}

	let { options, selected, onchange, placeholder = 'Any', label }: Props = $props();

	let open = $state(false);
	let query = $state('');
	let root: HTMLDivElement | undefined = $state();
	let searchEl: HTMLInputElement | undefined = $state();

	const filtered = $derived.by(() => {
		const q = query.trim().toLowerCase();
		return q ? options.filter((o) => o.toLowerCase().includes(q)) : options;
	});

	function toggle(value: string) {
		onchange(selected.includes(value) ? selected.filter((s) => s !== value) : [...selected, value]);
	}

	function remove(e: MouseEvent, value: string) {
		e.stopPropagation();
		onchange(selected.filter((s) => s !== value));
	}

	function setOpen(next: boolean) {
		open = next;
		if (!next) query = '';
	}

	$effect(() => {
		if (open) searchEl?.focus();
	});

	// Close when focus or a click lands outside the component.
	$effect(() => {
		if (!open) return;
		const onPointer = (e: PointerEvent) => {
			if (root && !root.contains(e.target as Node)) setOpen(false);
		};
		document.addEventListener('pointerdown', onPointer);
		return () => document.removeEventListener('pointerdown', onPointer);
	});

	function onSearchKey(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			setOpen(false);
		} else if (e.key === 'Enter') {
			// Enter picks the only (or first) match rather than submitting the search form.
			e.preventDefault();
			if (filtered.length) toggle(filtered[0]);
		}
	}
</script>

<div class="pill-select" bind:this={root}>
	<div
		class="pill-select-field input"
		class:open
		role="button"
		tabindex="0"
		aria-haspopup="listbox"
		aria-expanded={open}
		aria-label={label}
		onclick={() => setOpen(!open)}
		onkeydown={(e) => {
			if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setOpen(!open); }
			else if (e.key === 'Escape') setOpen(false);
		}}
	>
		{#if selected.length}
			{#each selected as value (value)}
				<span class="pill">
					{value}
					<button type="button" class="pill-remove" aria-label="Remove {value}" onclick={(e) => remove(e, value)}>×</button>
				</span>
			{/each}
		{:else}
			<span class="pill-select-placeholder">{placeholder}</span>
		{/if}
		<span class="pill-select-caret" aria-hidden="true">▾</span>
	</div>

	{#if open}
		<div class="pill-select-menu">
			<input
				bind:this={searchEl}
				class="input pill-select-search"
				placeholder="Filter…"
				aria-label="Filter {label.toLowerCase()} options"
				bind:value={query}
				onkeydown={onSearchKey}
			/>
			<div class="pill-select-options" role="listbox" aria-multiselectable="true" aria-label={label}>
				{#each filtered as value (value)}
					<button
						type="button"
						class="pill-select-option"
						class:checked={selected.includes(value)}
						role="option"
						aria-selected={selected.includes(value)}
						onclick={() => toggle(value)}
					>
						<span class="pill-select-check" aria-hidden="true">{selected.includes(value) ? '✓' : ''}</span>
						{value}
					</button>
				{:else}
					<div class="pill-select-empty">No matches</div>
				{/each}
			</div>
			{#if selected.length}
				<button type="button" class="pill-select-clear" onclick={() => onchange([])}>Clear</button>
			{/if}
		</div>
	{/if}
</div>

<style>
	.pill-select-field {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		min-height: 38px;
		height: auto;
		padding: 5px 28px 5px 8px;
		position: relative;
		cursor: pointer;
	}

	.pill-select-field.open {
		border-color: var(--accent);
	}

	.pill-select-placeholder {
		color: var(--text3, var(--text2));
		font-size: 0.85rem;
		padding-left: 2px;
	}

	.pill-select-caret {
		position: absolute;
		right: 10px;
		top: 50%;
		transform: translateY(-50%);
		color: var(--text2);
		font-size: 0.75rem;
	}

	.pill {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 2px 4px 2px 10px;
		border-radius: 20px;
		border: 1px solid var(--accent);
		background: var(--accent-glow);
		color: var(--accent-text);
		font-size: 0.78rem;
		font-weight: 700;
		line-height: 1.4;
	}

	.pill-remove {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 18px;
		height: 18px;
		border: none;
		border-radius: 50%;
		background: none;
		color: inherit;
		font-size: 0.95rem;
		line-height: 1;
		cursor: pointer;
		padding: 0;
	}

	.pill-remove:hover {
		background: color-mix(in srgb, var(--accent) 30%, transparent);
	}

	.pill-select-menu {
		margin-top: 6px;
		border: 1px solid var(--border2);
		border-radius: var(--radius);
		background: var(--surface2);
		padding: 6px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.pill-select-search {
		height: 32px;
		padding: 4px 10px;
		font-size: 0.82rem;
	}

	.pill-select-options {
		max-height: 220px;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
	}

	.pill-select-option {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 6px 8px;
		border: none;
		border-radius: 6px;
		background: none;
		color: var(--text);
		font-size: 0.84rem;
		text-align: left;
		cursor: pointer;
	}

	.pill-select-option:hover,
	.pill-select-option:focus-visible {
		background: var(--accent-glow);
	}

	.pill-select-option.checked {
		color: var(--accent-text);
		font-weight: 700;
	}

	.pill-select-check {
		width: 14px;
		flex-shrink: 0;
		color: var(--accent-text);
	}

	.pill-select-empty {
		padding: 6px 8px;
		color: var(--text2);
		font-size: 0.82rem;
	}

	.pill-select-clear {
		align-self: flex-end;
		border: none;
		background: none;
		color: var(--text2);
		font-size: 0.78rem;
		font-weight: 700;
		cursor: pointer;
		padding: 2px 4px;
	}

	.pill-select-clear:hover {
		color: var(--accent-text);
	}
</style>
