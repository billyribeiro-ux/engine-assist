<script lang="ts">
	// CR-003: the annotation errors that stop a pin (setup and re-pin), one row per error with
	// its stage, step ID, line and the parser's message (escaped spans, never HTML; SR-26).
	import { problemPlace } from '#lib/actions/words.ts';
	import type { AnnotationProblem } from '#lib/ipc/types.ts';
	import Text from '#lib/render/Text.svelte';

	let { problems }: { problems: AnnotationProblem[] } = $props();
</script>

<div class="problems" role="alert">
	<p class="lead">
		{problems.length === 1
			? 'An annotation error stops the pin.'
			: `${problems.length} annotation errors in Stages 1–2 and F.2 stop the pin.`}
		Fix the blueprint, then try again.
	</p>
	<ul aria-label="Annotation errors">
		{#each problems as p, i (i)}
			<li>
				<span class="where">
					<span>{problemPlace(p)}, line {p.diagnostic.line}</span>
					<span class="rule">{p.diagnostic.rule}</span>
				</span>
				<span class="msg"><Text spans={p.diagnostic.message} /></span>
			</li>
		{/each}
	</ul>
</div>

<style>
	.problems {
		display: grid;
		gap: var(--space-2);
		font-size: var(--text-12);
		color: var(--ink);
	}
	.lead {
		margin: 0;
		color: var(--fail);
	}
	ul {
		margin: 0;
		padding: 0;
		list-style: none;
		display: grid;
		gap: var(--space-2);
	}
	li {
		display: grid;
		gap: var(--space-1);
		padding-left: var(--space-3);
		border-left: 2px solid var(--fail);
	}
	.where {
		font-weight: 600;
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		align-items: baseline;
	}
	.rule {
		font-family: var(--font-mono);
		font-weight: 400;
		color: var(--ink-muted);
	}
	.msg {
		overflow-wrap: anywhere;
	}
</style>
