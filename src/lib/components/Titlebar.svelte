<script lang="ts">
	// Title bar (components.md "Layout"): title, subtitle (path and branch), toolbar.
	import { current } from '#lib/stores/project.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import Icon from './Icon.svelte';
	import ProjectMenu from './ProjectMenu.svelte';
</script>

<header class="titlebar">
	<span class="titlebar-title">{ui.chrome.title}</span>
	{#if ui.chrome.sub}<span class="titlebar-sub">{ui.chrome.sub}</span>{/if}
	<span class="spacer"></span>
	{#if current.project}<ProjectMenu project={current.project.summary} />{/if}
	<button
		class="btn ghost icon-only"
		type="button"
		aria-label="Commands and search (⌘K)"
		title="Commands and search ⌘K"
		onclick={() => (ui.paletteOpen = true)}><Icon name="search" /></button
	>
	{#if ui.chrome.drawer}
		<button
			class="btn ghost icon-only"
			type="button"
			aria-label="Toggle terminal drawer (⌃`)"
			aria-pressed={ui.drawerOpen}
			title="Terminal ⌃`"
			onclick={() => ui.toggleDrawer()}><Icon name="drawer" /></button
		>
	{/if}
	{#if ui.chrome.inspector}
		<button
			class="btn ghost icon-only"
			type="button"
			aria-label="Toggle inspector (⌘⌥I)"
			aria-pressed={ui.inspectorOpen}
			title="Inspector ⌘⌥I"
			onclick={() => ui.toggleInspector()}><Icon name="inspector" /></button
		>
	{/if}
</header>
