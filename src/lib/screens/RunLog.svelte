<script lang="ts">
	// Run log (F-026, mockup run-log.html): the metrics strip (time to start per surface, p95 against
	// the 10 s target, manual prompt edits), search and outcome filter, and each launch with what it
	// ran with, its outcome and an action: Show terminal, the exact command (prompt as its hash,
	// SR-52), Resume while the transcript exists, or a fresh chain when it doesn't.
	import { goto } from '$app/navigation';

	import { report, runPrepared, runStep } from '#lib/actions/run.ts';
	import { failureText } from '#lib/actions/words.ts';
	import Icon from '#lib/components/Icon.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { RunSummary } from '#lib/ipc/types.ts';
	import { to } from '#lib/nav/routes.ts';
	import { RUN } from '#lib/nav/status.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { terminal } from '#lib/stores/terminal.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	ui.setChrome({ title: 'Run log', sub: 'All projects' });

	let query = $state('');
	let debounced = $state('');
	let timer: ReturnType<typeof setTimeout> | undefined;
	$effect(() => {
		const q = query;
		clearTimeout(timer);
		timer = setTimeout(() => (debounced = q), 200);
	});
	type Outcome = 'all' | 'running' | 'failed' | 'interrupted';
	let outcome = $state<Outcome>('all');
	let openId = $state<string | null>(null);

	const metrics = resource(
		() => null,
		(c) => c.metricsView({ projectId: null }),
		() => ['runs']
	);
	const page = resource(
		() => debounced,
		(c, q) => c.runsSearch({ query: q, projectId: null, cursor: null, limit: 100 }),
		() => ['runs']
	);
	const runs: RunSummary[] = $derived(
		(page.value?.runs ?? []).filter((r) =>
			outcome === 'all'
				? true
				: outcome === 'running'
					? r.status === 'running' || r.status === 'idle' || r.status === 'blocked'
					: outcome === 'failed'
						? r.status === 'errored' || (r.status === 'exited' && (r.exitCode ?? 0) !== 0)
						: r.status === 'interrupted'
		)
	);
	const m = $derived(metrics.value);
	const sec = (ms: number) => `${(ms / 1000).toFixed(1)} s`;
	const min = (ms: number) => `${(ms / 60_000).toFixed(1)} min`;
	const cli = $derived(m?.perSurface.find((s) => s.surface === 'cli'));
	const chat = $derived(m?.perSurface.find((s) => s.surface === 'chat'));
	const edits = $derived(m?.manualEditsByStage.reduce((n, s) => n + s.edits, 0) ?? 0);

	function when(iso: string): string {
		const d = new Date(iso);
		const today = '2026-10-19';
		const time = d.toISOString().slice(11, 16);
		if (iso.startsWith(today)) return `Today ${time}`;
		return `${d.getUTCDate()} ${d.toLocaleString('en-GB', { month: 'short', timeZone: 'UTC' })} ${time}`;
	}
	function argv(r: RunSummary): string {
		return r.argv.map((a) => (/\s/.test(a) ? `"${a}"` : a)).join(' ');
	}
	async function exportMetrics(): Promise<void> {
		const c = await ipc();
		try {
			const r = await c.metricsExport({});
			if (r.kind === 'saved') ui.toast(`Metrics exported to ${r.displayPath}`);
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	async function resume(r: RunSummary): Promise<void> {
		const c = await ipc();
		try {
			const prep = await c.chainResumePrepare({ chain: r.chain });
			report(
				await runPrepared(
					{ projectId: r.project, step: r.step ?? '', surface: 'cli', entry: 'stageView' },
					prep
				)
			);
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	async function showTerminal(r: RunSummary): Promise<void> {
		terminal.active = r.runId;
		const proj = await ipc().then((c) => c.projectOpen({ projectId: r.project }));
		await goto(to.stage(r.project, proj.summary.currentStage ?? 1));
		if (!ui.drawerOpen) ui.toggleDrawer();
	}
	const OUTCOMES: { value: Outcome; label: string }[] = [
		{ value: 'all', label: 'All' },
		{ value: 'running', label: 'Running' },
		{ value: 'failed', label: 'Failed' },
		{ value: 'interrupted', label: 'Interrupted' }
	];
</script>

<main class="main" id="main" aria-labelledby="rl-title">
	<div class="page wide">
		<h1 class="page-title" id="rl-title">Run log</h1>
		<p class="page-sub">
			Every launch, with the exact command and what happened. Resume works while Claude Code still
			has the conversation.
		</p>
		<div class="metrics" role="group" aria-label="Time to start a step and active time per step">
			{#if m && m.runsCounted > 0}
				<div class="metric">
					<b>{cli ? sec(cli.medianTimeToStartMs) : '—'}</b><span
						>Median time to start, CLI{cli ? ` (p95 ${sec(cli.p95TimeToStartMs)})` : ''}</span
					>
				</div>
				<div class="metric">
					<b>{chat ? sec(chat.medianTimeToStartMs) : '—'}</b><span
						>Median time to start, chat{chat ? ` (p95 ${sec(chat.p95TimeToStartMs)})` : ''}</span
					>
				</div>
				<div class="metric">
					<b class={[m.p95AllMs > 10_000 && 'over']}>{sec(m.p95AllMs)}</b><span
						>p95, all steps (target 10 s)</span
					>
				</div>
				<div class="metric">
					<b>{cli && cli.medianActiveMsPerStep > 0 ? min(cli.medianActiveMsPerStep) : '—'}</b><span
						>Median active time per step, CLI</span
					>
				</div>
				<div class="metric">
					<b>{chat && chat.medianActiveMsPerStep > 0 ? min(chat.medianActiveMsPerStep) : '—'}</b
					><span>Median active time per step, chat</span>
				</div>
				<div class="metric">
					<b>{edits}</b><span>Manual prompt edits</span>
				</div>
				<div class="metric">
					<b>{m.stagesThroughForeman}</b><span>Stages run through Foreman</span>
				</div>
				{#if m.baseline}
					<div class="metric">
						<b>{sec(m.baseline.foremanMedianMs)} vs {sec(m.baseline.baselineMedianMs)}</b><span
							>Median to start, Foreman vs by hand</span
						>
					</div>
				{/if}
			{:else if m}
				<div class="metric">
					<b>0</b><span>Runs counted. Metrics start with your first launch.</span>
				</div>
			{/if}
			<button class="btn export" type="button" onclick={exportMetrics}>Export metrics</button>
		</div>
		{#if m && m.manualEditsByStage.length > 0}
			<p class="muted small" id="rl-edits">
				Manual prompt edits by stage: {m.manualEditsByStage
					.map((s) => `Stage ${s.stage} · ${s.edits}`)
					.join(', ')}
			</p>
		{/if}
		<div class="filters" role="search">
			<label class="vh" for="rl-q">Search runs</label>
			<input
				class="input q"
				id="rl-q"
				bind:value={query}
				placeholder="Search step, session, or command"
			/>
			<Segmented
				options={OUTCOMES}
				value={outcome}
				label="Outcome"
				onchange={(v: Outcome) => (outcome = v)}
			/>
			<span class="muted small" role="status">{runs.length} of {page.value?.total ?? 0} runs</span>
		</div>
		{#if page.error}
			<p class="err" role="alert"><Icon name="cross" size="glyph" />{failureText(page.error)}</p>
		{:else if page.value && runs.length === 0}
			<div class="empty">
				<strong>No runs match</strong>
				<span
					>{query
						? `Nothing matches “${query}”. Try a step number or a session name.`
						: 'Runs appear here after your first launch.'}</span
				>
			</div>
		{:else}
			<table class="table runs">
				<thead>
					<tr>
						<th scope="col">Started</th><th scope="col">Project</th><th scope="col">Step</th><th
							scope="col">Session</th
						><th scope="col">Ran with</th><th scope="col">Outcome</th><th scope="col">To start</th
						><th scope="col"><span class="vh">Actions</span></th>
					</tr>
				</thead>
				<tbody>
					{#each runs as r (r.runId)}
						{@const look = RUN[r.status]}
						{@const isOpen = openId === r.runId}
						<tr class={[isOpen && 'is-selected']}>
							<td>{when(r.startedAt)}</td>
							<td>{r.projectName}</td>
							<td>{r.step && !r.step.includes(':') ? `${r.step} ` : ''}{r.stepTitle}</td>
							<td><code>{r.tabName}</code></td>
							<td
								>{r.ranWith}{#if r.substituted}<span class="muted sub">({r.substituted})</span
									>{/if}</td
							>
							<td>
								<span
									class={[
										'status',
										r.status === 'exited' && (r.exitCode ?? 0) === 0
											? 'pass'
											: r.status === 'errored' || ((r.exitCode ?? 0) !== 0 && r.status === 'exited')
												? 'fail'
												: r.status === 'running'
													? 'accent'
													: 'muted'
									]}
									><Icon
										name={r.status === 'exited' && (r.exitCode ?? 0) !== 0 ? 'cross' : look.icon}
										size="glyph"
									/>{r.status === 'exited'
										? `Exited ${r.exitCode ?? 0}`
										: look.word.charAt(0).toUpperCase() + look.word.slice(1)}</span
								>
								{#if r.error}<div class="muted tiny">{r.error}</div>{/if}
							</td>
							<td>{r.timeToStartMs === null ? '—' : sec(r.timeToStartMs)}</td>
							<td class="act">
								{#if r.status === 'running' || r.status === 'idle' || r.status === 'blocked'}
									<button class="btn ghost" type="button" onclick={() => showTerminal(r)}
										>Show terminal</button
									>
								{:else if r.status === 'interrupted' && !r.resumable}
									<span class="reason">{r.whyNotResumable}</span>
									{#if r.step}<button
											class="btn ghost"
											type="button"
											onclick={() =>
												runStep({
													projectId: r.project,
													step: r.step ?? '',
													surface: 'cli',
													entry: 'stageView',
													mode: 'freshWithContext'
												}).then(report)}>Start a fresh chain with context</button
										>{/if}
								{:else if r.resumable && r.status !== 'errored'}
									<button class="btn ghost" type="button" onclick={() => resume(r)}>Resume</button>
								{/if}
								<button
									class="btn ghost"
									type="button"
									aria-expanded={isOpen}
									aria-controls="argv-{r.runId}"
									onclick={() => (openId = isOpen ? null : r.runId)}
									>{isOpen ? 'Hide command' : 'Show command'}</button
								>
							</td>
						</tr>
						{#if isOpen}
							<tr class="argv-row is-selected" id="argv-{r.runId}">
								<td></td>
								<td colspan="7"><pre>{argv(r)}</pre></td>
							</tr>
						{/if}
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</main>

<style>
	.wide {
		max-width: 1180px;
	}
	.filters {
		display: flex;
		gap: var(--space-3);
		align-items: center;
		margin-top: var(--space-5);
		flex-wrap: wrap;
	}
	.q {
		width: 320px;
	}
	.metrics {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-4) var(--space-7);
		margin-top: var(--space-5);
		padding: var(--space-4) 0;
		border-top: var(--hairline) solid var(--rule);
		border-bottom: var(--hairline) solid var(--rule);
	}
	.metric {
		display: grid;
		gap: var(--space-1);
	}
	.metric b {
		font-size: var(--text-18);
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
	}
	.metric b.over {
		color: var(--attention);
	}
	.metric span {
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.export {
		align-self: center;
		margin-left: auto;
	}
	.runs {
		margin-top: var(--space-4);
	}
	.runs code {
		font-size: var(--text-12);
		white-space: nowrap;
	}
	.argv-row td {
		padding-top: 0;
	}
	.argv-row pre {
		margin: 0;
		padding: var(--space-3) var(--space-4);
		background: var(--well);
		border: var(--hairline) solid var(--rule);
		font-family: var(--font-mono);
		font-size: var(--text-12);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
	.act {
		white-space: nowrap;
	}
	.small {
		font-size: var(--text-12);
	}
	.sub {
		margin-left: var(--space-2);
	}
	.tiny {
		font-size: var(--text-11);
	}
</style>
