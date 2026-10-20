<script lang="ts">
	// Renders repo, pasted or terminal text as text (SR-26): typed spans from Rust, and every plain
	// string split again so bidi, zero-width and control characters show as visible escapes
	// (AC-XC-32). There is no HTML path: no {@html}, no innerHTML.
	import type { TextSpan } from '#lib/ipc/types.ts';

	import { visibleSpans } from './escape.ts';
	import Text from './Text.svelte';

	let {
		spans = null,
		text = null,
		layout = true
	}: { spans?: readonly TextSpan[] | null; text?: string | null; layout?: boolean } = $props();

	const list: readonly TextSpan[] = $derived(spans ?? visibleSpans(text ?? '', layout));
</script>

{#each list as s, i (i)}
	{#if s.kind === 'plain'}
		{#each visibleSpans(s.text, layout) as p, j (j)}{#if p.kind === 'escape'}<span
					class="esc"
					title="Hidden character {p.label}"
					role="img"
					aria-label="hidden character {p.label}">{p.label}</span
				>{:else if p.kind === 'plain'}{p.text}{/if}{/each}
	{:else if s.kind === 'escape'}<span
			class="esc"
			title="Hidden character {s.label}"
			role="img"
			aria-label="hidden character {s.label}">{s.label}</span
		>{:else if s.kind === 'code'}<code><Text text={s.text} {layout} /></code
		>{:else if s.kind === 'strong'}<strong><Text spans={s.children} {layout} /></strong
		>{:else if s.kind === 'emph'}<em><Text spans={s.children} {layout} /></em
		>{:else if s.kind === 'link'}<span class="link-text"><Text text={s.text} {layout} /></span>
		<span class="muted">(<Text text={s.url} layout={false} />)</span>{/if}
{/each}
