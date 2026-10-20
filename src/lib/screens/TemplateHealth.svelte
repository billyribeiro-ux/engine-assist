<script lang="ts">
	// Template health (F-003, mockup template-health.html): what Foreman can run from the pinned
	// blueprint: counts, the stages table, and every error and warning grouped by stage with its line.
	import { failureText } from '#lib/actions/words.ts';
	import Icon from '#lib/components/Icon.svelte';
	import type { Diagnostic, StageHealth, StepHealth } from '#lib/ipc/types.ts';
	import Text from '#lib/render/Text.svelte';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	const health = resource(
		() => null,
		(c) => c.templateHealth({ projectId: null }),
		() => ['settings']
	);
	const h = $derived(health.value);
	$effect(() => {
		ui.setChrome({ title: 'Template health', sub: h?.file ?? '' });
	});
	const LOOK: Record<
		StageHealth['status'],
		{ icon: 'check' | 'warn' | 'cross' | 'dashsq'; cls: string }
	> = {
		runnable: { icon: 'check', cls: 'pass' },
		warning: { icon: 'warn', cls: 'attention' },
		partial: { icon: 'cross', cls: 'fail' },
		needsAnnotation: { icon: 'dashsq', cls: 'muted' }
	};
	const byStage = $derived.by(() => {
		const m: { stage: number; list: Diagnostic[] }[] = [];
		for (const d of [...(h?.diagnostics ?? [])].sort((a, b) => b.stage - a.stage)) {
			const g = m.find((x) => x.stage === d.stage);
			if (g) g.list.push(d);
			else m.push({ stage: d.stage, list: [d] });
		}
		return m;
	});
	// AC-F003-05: every runnable step with its annotation, lint warnings and declared outputs,
	// grouped by stage; a stage's list opens on request (all closed at first, the long view).
	const stepsByStage = $derived.by(() => {
		const m: { stage: number; list: StepHealth[] }[] = [];
		for (const st of h?.steps ?? []) {
			const g = m.find((x) => x.stage === st.stage);
			if (g) g.list.push(st);
			else m.push({ stage: st.stage, list: [st] });
		}
		return m;
	});
	let open = $state<Record<number, boolean>>({});
	// AC-F003-06: a template with no errors or warnings says so instead of empty groups.
	const spotless = $derived(!!h && h.counts.errors === 0 && h.counts.warnings === 0);
	const clean = $derived(
		h
			? h.stages
					.filter((s) => s.status === 'runnable')
					.map((s) => Number(s.stages))
					.filter((n) => !byStage.some((g) => g.stage === n))
			: []
	);
</script>

<main class="main" id="main" aria-labelledby="th-title">
	<div class="page wide">
		<h1 class="page-title" id="th-title">Template health</h1>
		{#if health.error}
			<p class="err" role="alert"><Icon name="cross" size="glyph" />{failureText(health.error)}</p>
		{:else if h}
			<p class="page-sub">
				What Foreman can run from blueprint {h.version}. Only <code>fm:</code> annotations decide behavior;
				the prose is checked against them and only warns.
			</p>
			<dl class="counts">
				<div>
					<dt>Blueprint, {h.commit}, grammar v{h.grammarVersion}</dt>
					<dd>{h.version}</dd>
				</div>
				<div>
					<dt>Stages</dt>
					<dd>{h.counts.stages}</dd>
				</div>
				<div>
					<dt>Runnable steps</dt>
					<dd>{h.counts.runnableSteps}</dd>
				</div>
				<div>
					<dt>Inspection items</dt>
					<dd>{h.counts.items}</dd>
				</div>
				<div>
					<dt>{h.counts.errors === 1 ? 'Error' : 'Errors'}</dt>
					<dd class={[h.counts.errors && 'fail']}>{h.counts.errors}</dd>
				</div>
				<div>
					<dt>{h.counts.warnings === 1 ? 'Warning' : 'Warnings'}</dt>
					<dd class={[h.counts.warnings && 'attention']}>{h.counts.warnings}</dd>
				</div>
			</dl>
			<div class="cols">
				<section class="section" aria-labelledby="st-h">
					<div class="section-head"><h2 class="section-title" id="st-h">Stages</h2></div>
					<table class="table health">
						<thead
							><tr
								><th scope="col"><span class="vh">Number</span></th><th scope="col">Stage</th><th
									scope="col">Status</th
								><th scope="col">Steps</th></tr
							></thead
						>
						<tbody>
							{#each h.stages as s (s.stages)}
								{@const l = LOOK[s.status]}
								<tr class={[s.status === 'needsAnnotation' && 'annot']}>
									<td>{s.stages}</td>
									<td>{s.title}</td>
									<td
										><span class="status {l.cls}"><Icon name={l.icon} size="glyph" />{s.text}</span
										></td
									>
									<td>{s.steps}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</section>
				<section class="section" aria-labelledby="dg-h">
					<div class="section-head">
						<h2 class="section-title" id="dg-h">Errors and warnings</h2>
					</div>
					{#if spotless}
						<p class="empty none" role="status">
							<strong>No errors or warnings.</strong>Every annotation in blueprint {h.version} parsed cleanly
							and the prose agrees with it.
						</p>
					{/if}
					{#each byStage as g (g.stage)}
						<h3 class="field-label grp">Stage {g.stage}</h3>
						<ul class="diag">
							{#each g.list as d, i (i)}
								<li>
									<Icon
										name={d.severity === 'error' ? 'cross' : 'warn'}
										size="glyph"
										tone={d.severity === 'error' ? 'fail' : 'attention'}
									/>
									<span class="loc">{d.line}:{d.column}<span class="vh"> {d.severity}</span></span>
									<span><Text spans={d.message} /></span>
								</li>
							{/each}
						</ul>
					{/each}
					{#if clean.length && !spotless}
						<h3 class="field-label grp">Stages {clean[0]} to {clean.at(-1)}</h3>
						<p class="empty none">No errors or warnings.</p>
					{/if}
					<p class="remap">Foreman proposes fixes; you edit the blueprint and re-pin.</p>
				</section>
			</div>
			<section class="section" aria-labelledby="rs-h">
				<div class="section-head">
					<h2 class="section-title" id="rs-h">Runnable steps</h2>
				</div>
				{#if stepsByStage.length === 0}
					<p class="empty">
						<strong>No runnable steps.</strong>Annotate a stage with <code>fm:step</code> to make its
						steps runnable.
					</p>
				{/if}
				{#each stepsByStage as g (g.stage)}
					{@const expanded = open[g.stage] ?? false}
					<h3 class="grp-h">
						<button
							type="button"
							class="disclosure"
							aria-expanded={expanded}
							aria-controls="rs-{g.stage}"
							onclick={() => (open[g.stage] = !expanded)}
							><Icon name={expanded ? 'chev-down' : 'chev'} />Stage {g.stage}
							<span class="meta"
								>{g.list.length} runnable step{g.list.length === 1 ? '' : 's'}{g.list.some(
									(x) => x.warnings.length
								)
									? ', with warnings'
									: ''}</span
							></button
						>
					</h3>
					<ul class="steps" id="rs-{g.stage}" hidden={!expanded}>
						{#each g.list as st (st.key)}
							<li>
								<p class="step-name"><span class="num">{st.number}</span><span>{st.title}</span></p>
								<dl class="kv">
									<dt>Annotation</dt>
									<dd><code class="annot">{st.annotation}</code></dd>
									<dt>Outputs</dt>
									<dd>
										{#if st.outs.length}
											<ul class="outs">
												{#each st.outs as o (o)}<li><code>{o}</code></li>{/each}
											</ul>
										{:else}
											<span class="muted">None declared</span>
										{/if}
									</dd>
									<dt>Lint</dt>
									<dd>
										{#if st.warnings.length}
											<ul class="outs">
												{#each st.warnings as w, i (i)}
													<li class="warn">
														<Icon name="warn" size="glyph" tone="attention" /><span class="loc"
															>{w.line}:{w.column}</span
														>
														<span><Text spans={w.message} /></span>
													</li>
												{/each}
											</ul>
										{:else}
											<span class="muted">No warnings</span>
										{/if}
									</dd>
								</dl>
							</li>
						{/each}
					</ul>
				{/each}
			</section>
		{:else if health.loading}
			<p class="page-sub" role="status">Reading the blueprint…</p>
		{/if}
	</div>
</main>

<style>
	.wide {
		max-width: 1180px;
	}
	.counts {
		display: flex;
		gap: var(--space-7);
		margin-top: var(--space-5);
		padding: var(--space-4) 0;
		border-top: var(--hairline) solid var(--rule);
		border-bottom: var(--hairline) solid var(--rule);
	}
	.counts div {
		display: flex;
		flex-direction: column-reverse;
	}
	.counts dd {
		font-size: var(--text-18);
		font-weight: var(--weight-semibold);
	}
	.counts dd.fail {
		color: var(--fail);
	}
	.counts dd.attention {
		color: var(--attention);
	}
	.counts dt {
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.health td:first-child {
		width: 44px;
		white-space: nowrap;
		text-align: right;
		color: var(--ink-muted);
		font-variant-numeric: tabular-nums;
	}
	.health tr.annot td {
		color: var(--ink-muted);
	}
	.diag {
		display: grid;
	}
	.diag li {
		display: grid;
		grid-template-columns: 22px 64px minmax(0, 1fr);
		gap: var(--space-3);
		padding: var(--space-3) 0;
		border-bottom: var(--hairline) solid var(--rule);
		font-size: var(--text-12);
	}
	.loc {
		font-family: var(--font-mono);
		color: var(--ink-muted);
	}
	.cols {
		display: grid;
		grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
		gap: var(--space-7);
	}
	.grp {
		display: block;
		margin-top: var(--space-4);
	}
	.none {
		padding-top: var(--space-2);
	}
	.grp-h {
		margin-top: var(--space-3);
	}
	.disclosure {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) 0;
		border: 0;
		background: transparent;
		color: var(--ink);
		font: inherit;
		font-size: var(--text-13);
		font-weight: var(--weight-semibold);
		cursor: default;
	}
	.disclosure .meta {
		color: var(--ink-muted);
		font-weight: var(--weight-regular);
		font-size: var(--text-12);
	}
	.steps > li {
		padding: var(--space-3) 0;
		border-bottom: var(--hairline) solid var(--rule);
	}
	.step-name {
		display: flex;
		gap: var(--space-3);
		font-size: var(--text-13);
		margin-bottom: var(--space-2);
	}
	.num {
		color: var(--ink-muted);
		font-variant-numeric: tabular-nums;
		min-width: 32px;
	}
	.annot {
		font-family: var(--font-mono);
		overflow-wrap: anywhere;
	}
	.outs {
		display: grid;
		gap: var(--space-1);
	}
	.outs .warn {
		display: grid;
		grid-template-columns: 16px 64px minmax(0, 1fr);
		gap: var(--space-2);
	}
	.muted {
		color: var(--ink-muted);
	}
</style>
