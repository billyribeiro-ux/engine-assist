<script lang="ts">
	// An icon from the mockups' set (components.md). Decorative: hidden from assistive tech; the
	// control or status next to it carries the words (status is shape plus color plus text).
	import { ICONS, type IconName, type IconNode } from './icons.ts';

	let {
		name,
		size = 'md',
		tone = null,
		class: cls = '',
		landing = false
	}: {
		name: IconName;
		size?: 'md' | 'sm' | 'glyph' | 'none';
		tone?: 'pass' | 'fail' | 'attention' | 'muted' | 'accent' | null;
		class?: string;
		/** The stamp's one orchestrated motion (components.md "The stamp"). */
		landing?: boolean;
	} = $props();

	const nodes: readonly IconNode[] = $derived(ICONS[name]);
</script>

{#snippet draw(list: readonly IconNode[])}
	{#each list as n, i (i)}
		{#if n.t === 'g'}
			<g {...n.a} class={[n.a.class, landing && n.a.class === 'stamp' && 'is-landing']}
				>{@render draw(n.c ?? [])}</g
			>
		{:else}
			<svelte:element this={n.t} xmlns="http://www.w3.org/2000/svg" {...n.a} />
		{/if}
	{/each}
{/snippet}

<svg
	class={[
		size !== 'none' && 'icon',
		size === 'sm' && 'sm',
		size === 'glyph' && 'glyph',
		tone && `glyph-${tone}`,
		cls
	]}
	viewBox="0 0 16 16"
	aria-hidden="true"
	focusable="false"
>
	{@render draw(nodes)}
</svg>
