<script lang="ts">
	// Title block (components.md, AC-F030-07): drafting box with sheet, project, revision, DRI and
	// status (in progress with a count, or the signed date with a stamp glyph). Not focusable; values
	// truncate with a tooltip, and the full text is in the accessible name.
	import type { ProjectView, StageView } from '#lib/ipc/types.ts';

	import Icon from './Icon.svelte';

	let { project, stage }: { project: ProjectView; stage: StageView } = $props();

	const applicable = $derived(
		stage.steps.filter((s) => s.state !== 'needsAnnotation' && s.state !== 'notApplicable')
	);
	const done = $derived(applicable.filter((s) => s.state === 'done' || s.state === 'skipped').length);
	const status = $derived(
		stage.signed
			? stage.signed.merged
				? `Signed ${stage.signed.date}`
				: `Signed on ${stage.signed.branch}, not merged`
			: stage.status === 'notApplicable'
				? 'Not applicable'
				: stage.status === 'needsAnnotation'
					? 'Needs annotation'
					: stage.status === 'locked'
						? 'Locked'
						: stage.status === 'inspecting' || stage.status === 'waiting'
							? `Inspecting, ${done} of ${applicable.length}`
							: `In progress, ${done} of ${applicable.length}`
	);
	const revision = $derived(`Blueprint ${project.blueprint.version}, ${project.blueprint.commit}`);
	const label = $derived(
		`Title block: sheet ${stage.stage} of 19, project ${project.summary.name}, revision ${revision}, DRI ${project.dri ?? 'not set'}, status ${status}`
	);
</script>

<div class="title-block" role="group" aria-label={label}>
	<div class="tb-sheet" aria-hidden="true">
		<span class="tb-label">Sheet</span><span class="tb-value"
			>{stage.stage}<span class="of">&nbsp;of 19</span></span
		>
	</div>
	<div aria-hidden="true">
		<span class="tb-label">Project</span><span class="tb-value" title={project.summary.name}
			>{project.summary.name}</span
		>
	</div>
	<div aria-hidden="true">
		<span class="tb-label">Revision</span><span class="tb-value" title={revision}>{revision}</span>
	</div>
	<div aria-hidden="true">
		<span class="tb-label">DRI</span><span class="tb-value" title={project.dri ?? ''}
			>{project.dri ?? '—'}</span
		>
	</div>
	<div aria-hidden="true">
		<span class="tb-label">Status</span><span class="tb-value" title={status}
			>{#if stage.signed}<Icon name="stamp" size="glyph" tone="pass" />{/if}{status}</span
		>
	</div>
</div>

<style>
	.title-block {
		display: grid;
		grid-template-columns: 70px 132px 150px;
		grid-template-rows: auto auto;
		border: var(--rule-strong) solid var(--ink-muted);
		font-family: var(--font-drafting);
		font-weight: var(--drafting-weight);
		font-stretch: var(--drafting-stretch);
		font-size: var(--text-12);
		line-height: 1.25;
	}
	.title-block > div {
		padding: 5px var(--space-3) 6px;
		border-left: var(--hairline) solid var(--rule);
		border-top: var(--hairline) solid var(--rule);
		min-width: 0;
	}
	.title-block > div:nth-child(-n + 3) {
		border-top: 0;
	}
	.title-block > .tb-sheet {
		grid-row: 1 / span 2;
		border-left: 0;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
	}
	.tb-label {
		display: block;
		color: var(--ink-muted);
		font-size: var(--text-11);
	}
	.tb-value {
		display: block;
		color: var(--ink);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.tb-sheet .tb-value {
		font-size: var(--text-28);
		line-height: 1;
	}
	.of {
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
</style>
