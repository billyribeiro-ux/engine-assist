<script lang="ts">
	// The P1 close-out checklist step (F-015 P1, AC-F015-01, MASTER-SPEC §8): once a stage's packet
	// is valid, a manual step lists the close-out sequence for its stage type. Phase 1 closes stages
	// by hand; the checks are reminders kept in this view, and nothing here signs or commits.
	import Icon from './Icon.svelte';

	let { stage, items }: { stage: number; items: string[] } = $props();
	let done = $state<Record<number, boolean>>({});
	const headId = $derived(`closeout-${stage}`);
	const count = $derived(items.filter((_, i) => done[i]).length);
</script>

<li class="step closeout" aria-labelledby={headId}>
	<span class="callout" aria-hidden="true"><Icon name="stamp" size="glyph" tone="muted" /></span>
	<div class="step-head">
		<h2 class="step-title" id={headId}>Close out Stage {stage}</h2>
		<span class="step-state">Manual · {count} of {items.length}</span>
	</div>
	<div class="step-body">
		<p class="hint">
			The packet is valid. Close the stage by hand, in this order; signing stays with you in this
			version.
		</p>
		<ol class="checklist" aria-label="Close-out sequence for Stage {stage}">
			{#each items as item, i (i)}
				<li>
					<button
						type="button"
						class="check"
						role="checkbox"
						aria-checked={done[i] ?? false}
						onclick={() => (done[i] = !(done[i] ?? false))}
						><span class={['box', done[i] && 'on']} aria-hidden="true"
							>{#if done[i]}<Icon name="check" size="none" />{/if}</span
						><span class="text">{item}</span></button
					>
				</li>
			{/each}
		</ol>
	</div>
</li>

<style>
	.closeout {
		display: grid;
		grid-template-columns: var(--step-margin) minmax(0, 1fr);
		padding: var(--space-4) var(--space-4) var(--space-4) 0;
		border-bottom: var(--hairline) solid var(--rule);
	}
	.callout {
		grid-row: 1 / span 2;
		justify-self: center;
		width: 34px;
		height: 34px;
		border-radius: 50%;
		border: var(--hairline) dashed var(--ink-muted);
		display: grid;
		place-items: center;
		margin-top: -6px;
	}
	.step-head {
		display: flex;
		align-items: baseline;
		gap: var(--space-4);
	}
	.step-title {
		font-size: var(--text-15);
		font-weight: var(--weight-semibold);
	}
	.step-state {
		margin-left: auto;
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.step-body {
		grid-column: 2;
		margin-top: var(--space-3);
		display: grid;
		gap: var(--space-3);
		max-width: var(--measure-prompt);
	}
	.hint {
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.checklist {
		display: grid;
		gap: var(--space-2);
		list-style: decimal;
		padding-left: var(--space-5);
	}
	.check {
		all: unset;
		display: inline-flex;
		gap: var(--space-3);
		align-items: baseline;
		cursor: default;
	}
	.check:focus-visible {
		outline: var(--focus-width) solid var(--focus);
		outline-offset: var(--focus-offset);
	}
	.check[aria-checked='true'] .text {
		color: var(--ink-muted);
	}
</style>
