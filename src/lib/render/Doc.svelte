<script lang="ts">
	// A typed Markdown node tree (ipc.md §2.3 DocNode) rendered with Svelte templates only (SR-26).
	import type { DocNode } from '#lib/ipc/types.ts';

	import Doc from './Doc.svelte';
	import Text from './Text.svelte';

	let { nodes }: { nodes: readonly DocNode[] } = $props();
</script>

{#each nodes as n, i (i)}
	{#if n.kind === 'paragraph'}
		<p class="doc-p"><Text spans={n.spans} /></p>
	{:else if n.kind === 'heading'}
		<p class="doc-h"><strong><Text spans={n.spans} /></strong></p>
	{:else if n.kind === 'list'}
		<svelte:element this={n.ordered ? 'ol' : 'ul'} class="doc-list">
			{#each n.items as item, j (j)}
				<li><Doc nodes={item} /></li>
			{/each}
		</svelte:element>
	{:else if n.kind === 'quote'}
		<blockquote class="doc-quote"><Doc nodes={n.children} /></blockquote>
	{:else if n.kind === 'codeBlock'}
		<code class="doc-code"><Text text={n.text} /></code>
	{:else}
		<hr />
	{/if}
{/each}

<style>
	.doc-list {
		display: grid;
		gap: var(--space-1);
	}
	.doc-quote {
		margin: 0;
		padding-left: var(--space-4);
		border-left: 2px solid var(--rule);
	}
	.doc-code {
		white-space: pre-wrap;
	}
</style>
