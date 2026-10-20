<script lang="ts">
	// Spec line (components.md, F-005): the run config as a small schedule. Every value says where it
	// came from on hover; edited values (substituted, alternative, override) are dotted and explain
	// themselves on hover or focus, e.g. "Fable 5.1 · high (Max) → Opus 5.5 · xhigh (substituted)".
	import type { Explained, SpecLine } from '#lib/ipc/types.ts';

	import Icon from './Icon.svelte';

	let { line, id }: { line: SpecLine; id: string } = $props();

	function why(e: Explained<string>, other: Explained<string> | null): string {
		switch (e.origin.kind) {
			case 'template':
				return 'From the template';
			case 'alternative':
				return `Alternative from the template: ${e.origin.rule}`;
			case 'override':
				return `${e.from ?? ''} → ${e.value} (your override for this run)`;
			case 'substitution': {
				const pair = other?.origin.kind === 'substitution' ? other : null;
				const from = pair ? `${e.from ?? ''} · ${pair.from ?? ''}` : (e.from ?? '');
				const to = pair ? `${e.value} · ${pair.value}` : e.value;
				return `${from} (Max) → ${to} (substituted)`;
			}
		}
	}
	const agents = $derived(
		line.agents.length > 2 ? `${line.agents.length} agents` : line.agents.join(', ')
	);
	const surfaceIcon = $derived(
		line.surface === 'cli' ? 'terminal' : line.surface === 'chat' ? 'chat' : line.surface === 'desktop' ? 'desktop' : 'play'
	);
</script>

{#snippet value(label: string, e: Explained<string>, other: Explained<string> | null, cls: string)}
	<div>
		<dt>{label}</dt>
		{#if e.origin.kind === 'template'}
			<dd class={cls} title="{label}: {why(e, other)}">{e.value}</dd>
		{:else}
			<dd class="tip-host">
				<button type="button" class="edited" aria-describedby="{id}-{label}">{e.value}</button>
				<span role="tooltip" id="{id}-{label}" class="tip">{why(e, other)}</span>
			</dd>
		{/if}
	</div>
{/snippet}

<dl class="spec-line" aria-label="Runs with">
	<div>
		<dt>Surface</dt>
		<dd><Icon name={surfaceIcon} size="sm" />{line.where}</dd>
	</div>
	{#if line.model}{@render value('Model', line.model, line.effort, 'model')}{/if}
	{#if line.effort}{@render value('Effort', line.effort, line.model, '')}{/if}
	{#if line.mode}{@render value('Mode', line.mode, null, '')}{/if}
	{#if !line.model && line.note}
		<div><dt>Writes</dt><dd class="model">{line.note}</dd></div>
		<div><dt>Model</dt><dd>—</dd></div>
	{:else if line.surface === 'chat' && !line.mode}
		<div><dt>Mode</dt><dd>—</dd></div>
	{/if}
	{#if agents}<div><dt>Agents</dt><dd>{agents}</dd></div>{/if}
	{#each line.droppedAgents as a (a)}<div><dt>Agent dropped</dt><dd>{a} not applicable</dd></div>{/each}
	{#if line.note && line.model}<div><dt>Note</dt><dd class="note">{line.note}</dd></div>{/if}
</dl>

<style>
	.tip-host {
		position: relative;
	}
	.tip {
		position: absolute;
		left: 0;
		top: calc(100% + 4px);
		z-index: 10;
		display: none;
		white-space: nowrap;
		padding: var(--space-2) var(--space-3);
		background: var(--sheet-raised);
		color: var(--ink);
		border-radius: var(--radius-control);
		box-shadow: var(--shadow-overlay);
		font-size: var(--text-12);
	}
	.tip-host:hover .tip,
	.tip-host:focus-within .tip {
		display: block;
	}
</style>
