<script lang="ts">
	// Rebuild from repo (F-020, AC-F020-11, AC-F020-12): offered when a project's state can't be
	// read. Review first lists what the rebuild finds per stage and what can't be recovered; the
	// state is written only when Billy presses Rebuild. Rust keeps an unreadable file beside the new
	// one, and refuses (Conflict) if the file changed after the review.
	import { failureText } from '#lib/actions/words.ts';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { ProjectView, RebuildPreview } from '#lib/ipc/types.ts';
	import { SHEET } from '#lib/nav/status.ts';
	import { refresh } from '#lib/stores/bus.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import { tick } from 'svelte';

	import Icon from './Icon.svelte';

	let { projectId, onrebuilt }: { projectId: string; onrebuilt?: (v: ProjectView) => void } =
		$props();

	let preview = $state<RebuildPreview | null>(null);
	let busy = $state(false);
	let error = $state<string | null>(null);
	let reviewHeading = $state<HTMLElement>();

	async function review(): Promise<void> {
		busy = true;
		error = null;
		try {
			const c = await ipc();
			preview = await c.progressRebuildPreview({ projectId });
			await tick();
			reviewHeading?.focus();
		} catch (e: unknown) {
			error = failureText(toIpcFailure(e));
		} finally {
			busy = false;
		}
	}
	async function apply(): Promise<void> {
		if (!preview) return;
		busy = true;
		error = null;
		try {
			const c = await ipc();
			const v = await c.progressRebuildApply({ previewId: preview.previewId });
			preview = null;
			ui.toast('Rebuilt from the repo');
			onrebuilt?.(v);
			refresh(`project:${projectId}`, 'projects', 'today');
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			preview = null;
			error =
				f.code === 'conflict'
					? 'The progress file changed after your review. Review again.'
					: f.code === 'expired'
						? 'That review expired. Review again.'
						: failureText(f);
		} finally {
			busy = false;
		}
	}
</script>

<section class="rebuild" aria-labelledby="rb-h">
	<h2 class="section-title" id="rb-h">Rebuild from repo</h2>
	{#if !preview}
		<p class="muted">
			Foreman can rebuild this project's stage and step status from its packets in the repo. You
			review the result first; nothing is written until you confirm.
		</p>
		<div class="actions top">
			<button class="btn" type="button" onclick={review} disabled={busy}
				><Icon name="restore" />Review the rebuild</button
			>
		</div>
	{:else}
		<h3 class="sub" tabindex="-1" bind:this={reviewHeading}>What the rebuild finds</h3>
		<ul class="stages" aria-label="Stages after the rebuild">
			{#each preview.stages as s (s.stage)}
				{@const done = s.steps.filter((x) => x.state === 'done').length}
				<li>
					<span class="num">{s.stage}</span><span class="title">{s.title}</span><span class="muted"
						>{SHEET[s.status].word}{done ? `, ${done} done` : ''}</span
					>
				</li>
			{/each}
		</ul>
		<h3 class="sub">Not recovered</h3>
		<ul class="lost">
			{#each preview.lost as l, i (i)}<li>{l}</li>{/each}
		</ul>
		<div class="actions top">
			<button class="btn primary" type="button" onclick={apply} disabled={busy}>Rebuild</button>
			<button class="btn ghost" type="button" onclick={() => (preview = null)} disabled={busy}
				>Cancel</button
			>
		</div>
	{/if}
	{#if error}<p class="err" role="alert"><Icon name="cross" size="glyph" />{error}</p>{/if}
</section>

<style>
	.rebuild {
		margin-top: var(--space-5);
		padding: var(--space-4);
		border: var(--hairline) solid var(--rule);
		display: grid;
		gap: var(--space-3);
	}
	.sub {
		font-size: var(--text-12);
		font-weight: var(--weight-semibold);
		margin: var(--space-3) 0 0;
	}
	.stages,
	.lost {
		margin: 0;
		padding: 0;
		list-style: none;
		display: grid;
		gap: var(--space-2);
		font-size: var(--text-12);
	}
	.stages li {
		display: grid;
		grid-template-columns: 28px minmax(0, 1fr) auto;
		gap: var(--space-3);
	}
	.num {
		font-family: var(--font-mono);
		color: var(--ink-muted);
	}
	.lost li::before {
		content: '– ';
		color: var(--ink-muted);
	}
	.top {
		margin-top: var(--space-3);
	}
</style>
