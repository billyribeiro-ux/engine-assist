<script lang="ts">
	// An icon from the mockups' set (components.md). Decorative: hidden from assistive tech; the
	// control or status next to it carries the words (status is shape plus color plus text).
	import { ICONS, type IconName, type IconNode } from './icons.ts';

	let {
		name,
		size = 'md',
		tone = null,
		class: cls = ''
	}: {
		name: IconName;
		size?: 'md' | 'sm' | 'glyph';
		tone?: 'pass' | 'fail' | 'attention' | 'muted' | 'accent' | null;
		class?: string;
	} = $props();

	const nodes: readonly IconNode[] = $derived(ICONS[name]);
</script>

{#snippet draw(list: readonly IconNode[])}
	{#each list as n, i (i)}
		{#if n.t === 'g'}
			<g {...n.a}>{@render draw(n.c ?? [])}</g>
		{:else}
			<svelte:element this={n.t} xmlns="http://www.w3.org/2000/svg" {...n.a} />
		{/if}
	{/each}
{/snippet}

<svg
	class={['icon', size === 'sm' && 'sm', size === 'glyph' && 'glyph', tone && `glyph-${tone}`, cls]}
	viewBox="0 0 16 16"
	aria-hidden="true"
	focusable="false"
>
	{@render draw(nodes)}
</svg>
