<script lang="ts">
	// An in-app review sheet (components.md "Sign dialog", diff sheets): role="dialog", modal, focus
	// starts inside and is trapped, Esc cancels and focus returns where it was. It only *requests* an
	// action; anything security-relevant is confirmed again by Rust in a native dialog (SR-24a).
	import type { Snippet } from 'svelte';
	import { onMount, tick } from 'svelte';

	let {
		title,
		description = null,
		onclose,
		children,
		footer,
		width = 'wide'
	}: {
		title: string;
		description?: string | null;
		onclose: () => void;
		children: Snippet;
		footer: Snippet;
		width?: 'wide' | 'narrow';
	} = $props();

	const id = `dlg-${Math.random().toString(36).slice(2, 8)}`;
	let box = $state<HTMLDivElement>();

	onMount(() => {
		const back = document.activeElement;
		const app = document.querySelector('.app');
		app?.setAttribute('inert', '');
		void tick().then(() => {
			const first = box?.querySelector<HTMLElement>(
				'[autofocus], input, textarea, select, button, [tabindex="0"]'
			);
			first?.focus();
		});
		return () => {
			app?.removeAttribute('inert');
			if (back instanceof HTMLElement) back.focus();
		};
	});

	function onKey(e: KeyboardEvent): void {
		if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			onclose();
			return;
		}
		if (e.key !== 'Tab' || !box) return;
		const f = [
			...box.querySelectorAll<HTMLElement>(
				'a[href], button:not([disabled]), input, textarea, select, [tabindex="0"]'
			)
		].filter((el) => !el.hasAttribute('disabled'));
		const first = f[0];
		const last = f.at(-1);
		if (!first || !last) return;
		if (e.shiftKey && document.activeElement === first) {
			e.preventDefault();
			last.focus();
		} else if (!e.shiftKey && document.activeElement === last) {
			e.preventDefault();
			first.focus();
		}
	}
</script>

<div class="scrim" aria-hidden="true"></div>
<div
	class={['dialog', width === 'narrow' && 'narrow']}
	role="dialog"
	aria-modal="true"
	aria-labelledby="{id}-t"
	aria-describedby={description ? `${id}-d` : undefined}
	tabindex="-1"
	bind:this={box}
	onkeydown={onKey}
>
	<div class="dialog-head">
		<h2 class="dialog-title" id="{id}-t">{title}</h2>
		{#if description}<p class="hashline" id="{id}-d">{description}</p>{/if}
	</div>
	<div class="dialog-body">{@render children()}</div>
	<div class="dialog-foot">{@render footer()}</div>
</div>

<style>
	.narrow {
		width: 520px;
	}
	.hashline {
		font-size: var(--text-12);
		color: var(--ink-muted);
		margin-top: var(--space-1);
	}
</style>
