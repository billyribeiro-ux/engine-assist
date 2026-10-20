<script lang="ts">
	// Stage workspace (F-030, mockups workspace.html and workspace-chat.html): stage header and
	// title block, banners (read-only, config changed), the steps as callouts with the selected
	// step's body, insert points (P2), the inspector, and the step shortcuts: J/K or ↓/↑ select,
	// Return expands, ⌘↵ runs, ⌘⇧C / ⌘⌥C copy, ⌘D marks a chat step done, ⌘⇧V pastes back.
	import { onMount, tick } from 'svelte';

	import { failureText } from '#lib/actions/words.ts';
	import CloseOutChecklist from '#lib/components/CloseOutChecklist.svelte';
	import Dialog from '#lib/components/Dialog.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import InsertPoint from '#lib/components/InsertPoint.svelte';
	import Inspector from '#lib/components/Inspector.svelte';
	import type { RowApi } from '#lib/components/row-api.ts';
	import StepRow from '#lib/components/StepRow.svelte';
	import TitleBlock from '#lib/components/TitleBlock.svelte';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { ProjectView, StepSummary } from '#lib/ipc/types.ts';
	import { registerShortcuts } from '#lib/keyboard/registry.ts';
	import Diff from '#lib/render/Diff.svelte';
	import { refresh } from '#lib/stores/bus.ts';
	import { current } from '#lib/stores/project.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	let {
		project,
		stageNo,
		stepKey = null
	}: { project: ProjectView; stageNo: number; stepKey?: string | null } = $props();

	const stage = $derived(project.stages.find((s) => s.stage === stageNo) ?? null);
	const steps: StepSummary[] = $derived(stage?.steps ?? []);
	const pid = $derived(project.summary.id);

	$effect(() => {
		ui.setChrome({
			title: project.summary.name,
			sub: `${project.summary.displayPath}, branch ${project.branch}`,
			drawer: true,
			inspector: true
		});
	});

	function defaultKey(): string | null {
		return (
			stepKey ??
			steps.find((s) => s.next)?.key ??
			steps.find((s) => s.state === 'copied' || s.state === 'notStarted')?.key ??
			steps.find((s) => s.state !== 'notApplicable' && s.state !== 'needsAnnotation')?.key ??
			null
		);
	}
	let selectedKey = $state<string | null>(null);
	let open = $state<string[]>([]);
	let seededFor = '';
	$effect(() => {
		const seed = `${pid}/${stageNo}/${stepKey ?? ''}`;
		if (seed === seededFor || steps.length === 0) return;
		seededFor = seed;
		const k = defaultKey();
		selectedKey = k;
		const extra = steps
			.filter((s) => s.tag === 'added' && s.state === 'notStarted')
			.map((s) => s.key);
		open = [...(k ? [k] : []), ...extra];
	});
	const selected = $derived(steps.find((s) => s.key === selectedKey) ?? null);
	$effect(() => {
		current.step = selected;
		current.stage = stageNo;
	});

	const rows: Record<string, RowApi | undefined> = {};
	const selectable = $derived(
		steps.filter((s) => s.state !== 'notApplicable' && s.state !== 'needsAnnotation')
	);

	async function select(d: number): Promise<true | string> {
		if (selectable.length === 0) return 'This stage has no runnable steps.';
		const i = selectable.findIndex((s) => s.key === selectedKey);
		const next = selectable[Math.max(0, Math.min(selectable.length - 1, (i < 0 ? -1 : i) + d))];
		if (!next) return true;
		selectedKey = next.key;
		await tick();
		document
			.querySelector<HTMLElement>(`[aria-controls="body-${next.key.replace(/[^a-z0-9]/gi, '-')}"]`)
			?.focus();
		return true;
	}
	function toggle(key: string): void {
		open = open.includes(key) ? open.filter((k) => k !== key) : [...open, key];
	}
	function needOpen(): RowApi | string {
		if (!selected) return 'Select a step first.';
		if (!open.includes(selected.key)) {
			open = [...open, selected.key];
			return 'Opening the step. Press the shortcut again to act on it.';
		}
		return rows[selected.key] ?? 'Select a step first.';
	}

	onMount(() =>
		registerShortcuts({
			nextStep: () => select(1),
			prevStep: () => select(-1),
			run: () => {
				const r = needOpen();
				return typeof r === 'string' ? r : r.run();
			},
			copyPrompt: () => {
				const r = needOpen();
				return typeof r === 'string' ? r : r.copy('prompt');
			},
			copyCommand: () => {
				const r = needOpen();
				return typeof r === 'string' ? r : r.copy('command');
			},
			pasteBack: async () => {
				if (selected?.surface !== 'chat')
					return 'Paste back works on a chat step that writes a packet or an output.';
				const r = needOpen();
				if (typeof r === 'string') {
					await tick();
					await tick();
					return rows[selected.key]?.paste() ?? r;
				}
				return r.paste();
			},
			markDone: () => {
				if (selected?.surface !== 'chat') return 'Mark done works on a chat step.';
				const r = needOpen();
				return typeof r === 'string' ? r : r.markDone();
			}
		})
	);

	// Config-changed banner (F-008): Acknowledge only requests the native acknowledgement in Rust.
	let reviewDiff = $state(false);
	async function acknowledge(): Promise<void> {
		const n = project.configChanged;
		if (!n) return;
		const c = await ipc();
		try {
			await c.configAck({ projectId: pid, cwd: n.cwd, fingerprint: n.fingerprint });
			ui.toast('Configuration change acknowledged');
			refresh(`project:${pid}`);
			await tick();
			document.querySelector<HTMLElement>('[aria-current="step"] .step-title button')?.focus();
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			ui.say(
				f.code === 'cancelled'
					? 'Cancelled. The change still needs your acknowledgement.'
					: failureText(f)
			);
		}
	}

	// Insert point (F-025, P2): the + between steps, in InsertPoint.
	const showInsert = (s: StepSummary) => s.state === 'running' || s.next || s.key === selectedKey;
	const stageWord = $derived(
		stage?.status === 'notApplicable'
			? `Not applicable: ${stage.applicable.kind === 'notApplicable' ? stage.applicable.reason : ''}`
			: stage?.status === 'locked'
				? `Locked until Stage ${stageNo - 1} is signed.`
				: stage?.status === 'needsAnnotation'
					? 'Shown but not runnable until a later blueprint annotates this stage.'
					: null
	);
</script>

{#if stage}
	<div class="body">
		<main class="main" id="main" aria-labelledby="stage-title">
			<div class="stage-head">
				<div>
					<h1 class="stage-title" id="stage-title">{stage.title}</h1>
					<p class="stage-sub">{stage.subtitle}</p>
				</div>
				<TitleBlock {project} {stage} />
			</div>

			{#if project.readOnly}
				<div class="banner fail" role="alert">
					<Icon name="lock" />
					<div>
						<span class="banner-title">This project is open read-only.</span>&#32;<span
							class="muted">{project.readOnly.detail}</span
						>
					</div>
					<div class="banner-actions"></div>
				</div>
			{/if}
			{#if project.configChanged}
				{@const n = project.configChanged}
				<div class={['banner', n.blocking && 'fail']} role={n.blocking ? 'alert' : 'status'}>
					<Icon name="warn" />
					<div>
						<span class="banner-title">{n.title}</span>&#32;<span class="muted">{n.detail}</span>
					</div>
					<div class="banner-actions">
						{#if n.diffs.length}<button
								class="btn"
								type="button"
								onclick={() => (reviewDiff = true)}>Review diff</button
							>{/if}
						{#if !n.blocking}<button class="btn" type="button" onclick={acknowledge}
								>Acknowledge</button
							>{/if}
					</div>
				</div>
			{/if}
			{#if stageWord}
				<p class="stage-note">
					<Icon
						name={stage.status === 'locked'
							? 'lock'
							: stage.status === 'notApplicable'
								? 'hatch'
								: 'dashsq'}
						size="glyph"
						tone="muted"
					/>{stageWord}
				</p>
			{/if}

			<ol class="steps" aria-label="Steps in Stage {stage.stage}">
				{#each steps as s (s.key)}
					<StepRow
						api={(a: RowApi | null) => (rows[s.key] = a ?? undefined)}
						{project}
						{stage}
						step={s}
						selected={s.key === selectedKey}
						expanded={open.includes(s.key)}
						full={s.key === selectedKey}
						onselect={() => (selectedKey = s.key)}
						ontoggle={() => toggle(s.key)}
					/>
					{#if showInsert(s) && s !== steps.at(-1) && !project.readOnly && s.state !== 'needsAnnotation'}
						<InsertPoint {project} {stage} after={s} />
					{/if}
				{/each}
				{#if stage.closeoutChecklist.length}
					<CloseOutChecklist stage={stage.stage} items={stage.closeoutChecklist} />
				{/if}
			</ol>
		</main>
		<Inspector
			{project}
			{stage}
			step={selected}
			initial={selected?.surface === 'chat' ? 'packet' : 'gate'}
		/>
	</div>
{:else}
	<main class="main" id="main">
		<div class="page"><h1 class="page-title">Stage {stageNo} doesn't exist</h1></div>
	</main>
{/if}

{#if reviewDiff && project.configChanged}
	<Dialog
		title="What changed in {project.configChanged.cwd === 'spec' ? 'spec/' : 'the project'}"
		description="Values are redacted. Acknowledge asks you once more in a macOS dialog."
		onclose={() => (reviewDiff = false)}
	>
		{#each project.configChanged.diffs as d (d.source)}
			<h3 class="insp-title"><code>{d.source}</code></h3>
			<Diff redacted={d} label="Redacted diff of {d.source}" />
		{/each}
		{#snippet footer()}
			<span class="spacer"></span>
			<button class="btn" type="button" onclick={() => (reviewDiff = false)}>Close</button>
			<button
				class="btn primary"
				type="button"
				onclick={() => ((reviewDiff = false), void acknowledge())}>Acknowledge</button
			>
		{/snippet}
	</Dialog>
{/if}

<style>
	.steps {
		padding: var(--space-3) var(--space-7) var(--space-8) var(--space-5);
	}
	.stage-note {
		display: flex;
		gap: var(--space-3);
		align-items: center;
		padding: var(--space-3) var(--space-7);
		font-size: var(--text-12);
		color: var(--ink-muted);
		border-bottom: var(--hairline) solid var(--rule);
	}
	@media (max-width: 1359px) {
		.steps {
			padding-right: var(--space-5);
		}
		.stage-head {
			padding-left: var(--space-6);
			padding-right: var(--space-5);
		}
	}
</style>
