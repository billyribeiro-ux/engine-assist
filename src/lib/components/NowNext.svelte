<script lang="ts">
	// Now/Next strip (components.md, F-009 P2): what the live session runs with, the next step, and
	// the round tracker. Announced once per change (role="status"), never per token.
	import type { ProjectView } from '#lib/ipc/types.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';

	import Icon from './Icon.svelte';

	let { project }: { project: ProjectView } = $props();

	const steps = $derived(project.stages.flatMap((s) => s.steps));
	const running = $derived(steps.find((s) => s.state === 'running') ?? null);
	const next = $derived(steps.find((s) => s.next) ?? null);
	const now = $derived(running?.specLine ?? null);

	const view = resource(
		() => (running ? { projectId: project.summary.id, step: running.key } : null),
		(c, a) => (a ? c.stepView(a) : Promise.resolve(null)),
		(a) => (a ? [`project:${a.projectId}`] : [])
	);
	const written = $derived(view.value?.outs.filter((o) => o.exists) ?? []);
</script>

<div class="nownext" role="status" aria-live="polite" aria-label="Now and next">
	{#if now}
		<span
			><b>Now</b>&nbsp; <span class="seg-val">{now.mode?.value ?? 'Manual'}</span>, {now.model
				?.value}, {now.effort?.value}</span
		>
		<span class="sep" aria-hidden="true"></span>
	{/if}
	{#if next}
		<span
			><b>Next</b>&nbsp; {next.number}
			{next.title}{next.specLine?.where.includes('continues')
				? ' re-arms this session when Claude is idle'
				: ''}</span
		>
	{:else}
		<span><b>Next</b>&nbsp; Nothing else waits on this session</span>
	{/if}
	{#each written as o (o.path)}
		<span class="rounds"
			><Icon name="check" size="glyph" tone="pass" />{o.path.split('/').at(-1)} written</span
		>
	{/each}
</div>

<style>
	.nownext {
		display: flex;
		align-items: center;
		gap: var(--space-5);
		padding: 6px var(--space-5);
		border-bottom: var(--hairline) solid var(--term-rule);
		font-size: var(--text-12);
		color: var(--term-muted);
		background: var(--term-bg);
		flex-wrap: wrap;
	}
	b {
		color: var(--term-ink);
		font-weight: var(--weight-semibold);
	}
	.seg-val {
		color: var(--term-ink);
	}
	.sep {
		width: 1px;
		height: 14px;
		background: var(--term-rule);
	}
	.rounds {
		margin-left: auto;
		display: inline-flex;
		gap: 6px;
		align-items: center;
	}
</style>
