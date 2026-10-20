<script lang="ts">
	// Now/Next strip (components.md, F-009 P2, AC-F009-11, -12): what the live session runs with
	// (with the P2 sensor, the live mode; a mismatch with the step's mode shows in the attention
	// state), the in-step cue or the next step, and the round tracker for a step with several
	// outputs, each written one offering Open output. Announced once per change (role="status"),
	// never per token.
	import { failureText } from '#lib/actions/words.ts';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { ProjectView } from '#lib/ipc/types.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import Icon from './Icon.svelte';

	let { project }: { project: ProjectView } = $props();

	const steps = $derived(project.stages.flatMap((s) => s.steps));
	const running = $derived(steps.find((s) => s.state === 'running') ?? null);
	const next = $derived(steps.find((s) => s.next) ?? null);
	const spec = $derived(running?.specLine ?? null);

	const view = resource(
		() =>
			running
				? { projectId: project.summary.id, step: running.key, rev: project.summary.rev }
				: null,
		(c, a) => (a ? c.stepView({ projectId: a.projectId, step: a.step }) : Promise.resolve(null)),
		(a) => (a ? [`project:${a.projectId}`] : [])
	);
	const live = $derived(view.value?.live ?? null);
	const mode = $derived(live?.mode ?? spec?.mode?.value ?? 'Manual');
	const now = $derived(
		spec ? [mode, spec.model?.value, spec.effort?.value].filter(Boolean).join(' · ') : null
	);
	const cue = $derived(view.value?.switches[0]?.text.replace(/^Next:\s*/, '') ?? null);
	const outs = $derived(view.value?.outs ?? []);
	const written = $derived(outs.filter((o) => o.exists));

	async function openOutput(path: string): Promise<void> {
		const c = await ipc();
		try {
			await c.fileOpen({ projectId: project.summary.id, path });
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
</script>

<div class="nownext" role="status" aria-live="polite" aria-label="Now and next">
	{#if now}
		<span class={['now', live?.expected && 'attention']}
			><b>Now</b>&nbsp; <span class="seg-val">{now}</span>{#if live?.expected}<span class="expected"
					><Icon name="warn" size="glyph" tone="attention" />{live.expected} expected</span
				>{/if}</span
		>
		<span class="sep" aria-hidden="true"></span>
	{/if}
	{#if running && cue}
		<span><b>Next</b>&nbsp; {cue}</span>
	{:else if next}
		<span
			><b>Next</b>&nbsp; {next.number}
			{next.title}{next.specLine?.where.includes('continues')
				? ' re-arms this session when Claude is idle'
				: ''}</span
		>
	{:else}
		<span><b>Next</b>&nbsp; Nothing else waits on this session</span>
	{/if}
	{#if outs.length > 1}
		<span class="rounds" aria-label="Outputs written: {written.length} of {outs.length}"
			><span class="count">{written.length} of {outs.length} written</span>
			{#each written as o (o.path)}
				<button
					class="btn ghost out"
					type="button"
					onclick={() => openOutput(o.path)}
					aria-label="Open output {o.path}"
					><Icon name="check" size="glyph" tone="pass" />{o.path.split('/').at(-1)}</button
				>
			{/each}
		</span>
	{:else if written[0]}
		<span class="rounds"
			><button
				class="btn ghost out"
				type="button"
				onclick={() => openOutput(written[0]?.path ?? '')}
				aria-label="Open output {written[0].path}"
				><Icon name="check" size="glyph" tone="pass" />{written[0].path.split('/').at(-1)} written</button
			></span
		>
	{/if}
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
	.attention .seg-val,
	.expected {
		color: var(--attention);
	}
	.expected {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		margin-left: var(--space-3);
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
		flex-wrap: wrap;
	}
	.out {
		color: var(--term-ink);
		font-size: var(--text-12);
		padding: 2px 6px;
	}
</style>
