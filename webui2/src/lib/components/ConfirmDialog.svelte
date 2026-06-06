<script lang="ts">
	interface Props {
		title: string;
		message: string;
		confirmLabel?: string;
		danger?: boolean;
		onconfirm: () => void;
		oncancel: () => void;
	}

	let { title, message, confirmLabel = 'Confirm', danger = false, onconfirm, oncancel }: Props = $props();
</script>

<div class="confirm-overlay" role="dialog" aria-modal="true">
	<div class="confirm-box">
		<h4>{title}</h4>
		<p>{message}</p>
		<div class="confirm-actions">
			<button class="btn" onclick={oncancel}>Cancel</button>
			<button class="btn" class:btn-danger={danger} class:btn-accent={!danger} onclick={onconfirm}>
				{confirmLabel}
			</button>
		</div>
	</div>
</div>
