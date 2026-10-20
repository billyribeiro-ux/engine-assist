<script lang="ts">
	// One shared polite live region and the toast stack (components.md "Toast"). Toasts stay 6 s,
	// longer while hovered or focused, and never hold the only copy of important information.
	import { ui } from '#lib/stores/ui.svelte.ts';

	import Icon from './Icon.svelte';
</script>

<div class="vh" role="status" aria-live="polite" aria-atomic="true">{ui.announcement}</div>
<div class={['toasts', ui.toastLift && 'lift']} aria-hidden="true">
	{#each ui.toasts as t (t.id)}
		<div class={['toast', t.tone === 'fail' && 'fail']}>
			<Icon name={t.icon} /><span>{t.text}</span>
		</div>
	{/each}
	{#if ui.hint}
		<div class="toast hint"><Icon name="info" /><span>{ui.hint}</span></div>
	{/if}
</div>

<style>
	.lift {
		bottom: 76px;
	}
	.hint > :global(.icon) {
		color: var(--ink-muted);
	}
</style>
