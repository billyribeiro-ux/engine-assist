<script lang="ts">
	// Prompt block (components.md, F-006): exactly the resolver's output, mono, read-only, wrapped at
	// ≤ 88 ch. Resolved values are dotted and say where they came from; unresolved required tokens
	// are attention pills that focus their input (AC-F006-13); optional parts are outlined. Text is
	// rendered from typed segments only (SR-26).
	import type { PromptSegment, RunInputView } from '#lib/ipc/types.ts';
	import Text from '#lib/render/Text.svelte';

	let {
		segments,
		stepNumber,
		inputs = [],
		inputId,
		caption = null
	}: {
		segments: PromptSegment[];
		stepNumber: string;
		inputs?: RunInputView[];
		inputId: (name: string) => string;
		caption?: string | null;
	} = $props();

	let expanded = $state(false);
	const id = $derived(`prompt-${stepNumber.replace(/[^a-z0-9]/gi, '-')}`);
	const lineCount = $derived(
		segments
			.map((s) =>
				s.kind === 'text'
					? s.spans.map((x) => ('text' in x ? x.text : '')).join('')
					: s.kind === 'resolved' || s.kind === 'optionalPart'
						? s.text
						: s.kind === 'unresolved'
							? s.token
							: ''
			)
			.join('')
			.split('\n').length
	);
	const clip = $derived(lineCount > 6 && !expanded);
	const hasPart = $derived(segments.some((s) => s.kind === 'optionalPart'));
	const label = (name: string | null) =>
		inputs.find((i) => i.name === name)?.label ?? name ?? 'the input';

	function focusInput(name: string | null): void {
		if (!name) return;
		document.getElementById(inputId(name))?.focus();
	}
</script>

<div class="prompt-wrap">
	<div class="prompt-cap">
		<span>Resolved prompt</span>
		<span class="remap"
			>{caption ??
				(hasPart ? 'The optional part is outlined' : 'Variables from .foreman/project.json')}</span
		>
	</div>
	<pre
		class={['prompt', clip && 'is-clipped']}
		{id}
		aria-label="Resolved prompt for {stepNumber}">{#each segments as s, i (i)}{#if s.kind === 'text'}<Text
					spans={s.spans}
				/>{:else if s.kind === 'resolved'}<span class="tok" title={s.note ?? s.token}
					><Text text={s.text} /></span
				>{:else if s.kind === 'unresolved'}<button
					type="button"
					class="tok-missing"
					aria-label="Fill {label(s.input)}"
					onclick={() => focusInput(s.input)}>{s.token}</button
				>{:else if s.kind === 'optionalPart'}<span class="part-optional"
					><Text text={s.text} /></span
				>{/if}{/each}</pre>
	{#if lineCount > 6}
		<button
			class="btn ghost prompt-more"
			type="button"
			aria-controls={id}
			aria-expanded={expanded}
			onclick={() => (expanded = !expanded)}
			>{expanded ? 'Show the first 6 lines' : `Show all ${lineCount} lines`}</button
		>
	{/if}
</div>

<style>
	.prompt-wrap {
		position: relative;
		max-width: calc(var(--measure-prompt) + 34px);
	}
	.prompt-cap {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		font-size: var(--text-12);
		color: var(--ink-muted);
		margin-bottom: 6px;
	}
	.prompt-more {
		font-size: var(--text-12);
	}
</style>
