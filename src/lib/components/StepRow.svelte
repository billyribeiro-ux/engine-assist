<script lang="ts">
	// A step as a callout on the sheet (components.md "Step", F-007, F-030): the callout number in
	// the margin, the title row (title, state, lock), the spec line, and for an expanded step its
	// body: run inputs, the optional-part toggle, the actions, the resolved prompt and the exact
	// command. Not-applicable, core, undecided and needs-annotation rows can't run and say why in one
	// line (AC-F030-02); core rows never show Skip (AC-F004-07).
	import { report, runStep } from '#lib/actions/run.ts';
	import { blockerText, failureText, MODE_NAME, MODEL_NAME } from '#lib/actions/words.ts';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type {
		ActionPreview,
		Effort,
		Mode,
		Model,
		ProjectView,
		RunOverride,
		StageView,
		StepSummary,
		StepView
	} from '#lib/ipc/types.ts';
	import { STEP } from '#lib/nav/status.ts';
	import Diff from '#lib/render/Diff.svelte';
	import Text from '#lib/render/Text.svelte';
	import { refresh } from '#lib/stores/bus.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { terminal } from '#lib/stores/terminal.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import Dialog from './Dialog.svelte';
	import Icon from './Icon.svelte';
	import PasteBack from './PasteBack.svelte';
	import PromptBlock from './PromptBlock.svelte';
	import type { RowApi } from './row-api.ts';
	import SpecLine from './SpecLine.svelte';

	let {
		project,
		stage,
		step,
		selected,
		expanded,
		full,
		onselect,
		ontoggle,
		api
	}: {
		project: ProjectView;
		stage: StageView;
		step: StepSummary;
		selected: boolean;
		expanded: boolean;
		/** The selected step shows its whole body; other open steps show inputs and actions. */
		full: boolean;
		onselect: () => void;
		ontoggle: () => void;
		/** Hands the workspace this row's shortcut actions (⌘↵, ⌘⇧C, ⌘⌥C, ⌘⇧V, ⌘D). */
		api?: (a: RowApi | null) => void;
	} = $props();

	const pid = $derived(project.summary.id);
	const sid = $derived(step.key.replace(/[^a-z0-9]/gi, '-'));
	const view = resource(
		() => (expanded ? { projectId: pid, step: step.key, rev: project.summary.rev } : null),
		(c, a) => (a ? c.stepView({ projectId: a.projectId, step: a.step }) : Promise.resolve(null)),
		(a) => (a ? [`project:${a.projectId}`] : [])
	);
	const v: StepView | null = $derived(view.value ?? null);

	const LOCK =
		'Core: protects production quality. Change it in the blueprint with a Change Request.';
	const quiet = $derived(['notApplicable', 'done', 'skipped'].includes(step.state));
	const isNa = $derived(step.state === 'notApplicable');
	const look = $derived(STEP[step.state]);
	const tone = $derived(
		step.state === 'done'
			? 'pass'
			: step.state === 'copied' || step.state === 'undecided'
				? 'attention'
				: step.state === 'running'
					? 'accent'
					: ''
	);
	const glyph = $derived(
		step.next && step.state === 'notStarted' && step.whyNotRunnable?.startsWith('Waits')
			? 'clock'
			: look.icon
	);
	const blocked = $derived(v ? v.blockers.length > 0 || !!project.readOnly : true);
	const reason = $derived(
		project.readOnly
			? `Open read-only: ${project.readOnly.detail}`
			: v?.blockers[0]
				? blockerText(v.blockers[0])
				: step.tag === 'added'
					? 'Added steps always confirm before running.'
					: null
	);
	const writesPacket = $derived(step.specLine?.note?.includes('packet') ?? false);
	const primary = $derived(
		step.kind === 'action'
			? { label: 'Preview and write', icon: 'file' as const }
			: step.surface === 'chat'
				? { label: 'Copy and open', icon: 'external' as const }
				: step.surface === 'desktop'
					? { label: 'Open in Desktop', icon: 'desktop' as const }
					: { label: 'Run in terminal', icon: 'play' as const }
	);
	const inputId = (name: string) => `in-${sid}-${name}`;

	let pasteTrigger: (() => Promise<true | string>) | null = null;
	let action = $state<ActionPreview | null>(null);
	let skipping = $state(false);
	let skipReason = $state('');
	let skipError = $state<string | null>(null);
	let busy = $state(false);

	$effect(() => {
		api?.({ run, copy, paste, markDone });
		return () => api?.(null);
	});

	async function run(): Promise<true | string> {
		if (!v) return 'Open the step first.';
		if (project.readOnly) return `Open read-only: ${project.readOnly.detail}`;
		if (v.blockers[0]) return blockerText(v.blockers[0]);
		if (step.kind === 'action') return preview();
		busy = true;
		const r = await runStep({
			projectId: pid,
			step: step.key,
			surface: step.surface,
			entry: 'stageView'
		});
		busy = false;
		return r;
	}
	async function copy(target: 'prompt' | 'command'): Promise<true | string> {
		if (target === 'command' && step.surface !== 'cli') return 'Only CLI steps have a command.';
		if (isNa || step.state === 'needsAnnotation')
			return step.whyNotRunnable ?? 'This step has no prompt.';
		const c = await ipc();
		try {
			const r = await c.clipboardCopy({
				projectId: pid,
				target,
				step: step.key,
				chain: null,
				path: null,
				stage: null
			});
			ui.toast(
				`${target === 'prompt' ? 'Prompt' : 'Command'} copied${r.removedControls ? `; removed ${r.removedControls} control characters` : ''}`,
				'pass',
				'copy'
			);
			if (r.focusRun) terminal.active = r.focusRun;
			refresh(`project:${pid}`);
			return true;
		} catch (e: unknown) {
			return failureText(toIpcFailure(e));
		}
	}
	function paste(): Promise<true | string> {
		if (step.surface !== 'chat')
			return Promise.resolve('Paste back works on a chat step that writes a packet or an output.');
		return pasteTrigger ? pasteTrigger() : Promise.resolve('Open the step first.');
	}
	async function markDone(): Promise<true | string> {
		if (step.surface !== 'chat') return 'Mark done works on a chat step.';
		const c = await ipc();
		try {
			view.set(await c.stepMarkDone({ projectId: pid, step: step.key }));
			ui.toast(`${step.number} marked done`);
			refresh(`project:${pid}`, 'today');
			return true;
		} catch (e: unknown) {
			return failureText(toIpcFailure(e));
		}
	}
	async function preview(): Promise<true | string> {
		const c = await ipc();
		try {
			action = await c.actionPreview({ projectId: pid, step: step.key });
			return true;
		} catch (e: unknown) {
			return failureText(toIpcFailure(e));
		}
	}
	async function applyAction(): Promise<void> {
		if (!action) return;
		const c = await ipc();
		try {
			view.set(await c.actionApply({ previewId: action.previewId }));
			ui.toast(`Wrote ${action.writes.length} files`);
			action = null;
			refresh(`project:${pid}`);
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}

	const timers: Record<string, ReturnType<typeof setTimeout>> = {};
	function setInput(name: string, value: string): void {
		clearTimeout(timers[name]);
		timers[name] = setTimeout(() => {
			void ipc().then(async (c) => {
				try {
					view.set(await c.runInputSet({ projectId: pid, step: step.key, name, value }));
				} catch (e: unknown) {
					ui.say(failureText(toIpcFailure(e)));
				}
			});
		}, 300);
	}
	async function setPart(partId: string, include: boolean): Promise<void> {
		const c = await ipc();
		try {
			view.set(await c.optionalPartSet({ projectId: pid, step: step.key, partId, include }));
			ui.announce(include ? 'Optional part included' : 'Optional part removed from the prompt');
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	async function skip(): Promise<void> {
		const c = await ipc();
		try {
			await c.stepSkip({ projectId: pid, step: step.key, reason: skipReason });
			skipping = false;
			ui.toast(`${step.number} skipped`);
			refresh(`project:${pid}`, 'today');
		} catch (e: unknown) {
			skipError = failureText(toIpcFailure(e));
		}
	}
	async function linkChat(): Promise<void> {
		const c = await ipc();
		try {
			await c.chatLinkSet({ projectId: pid, chainStart: step.key, source: { kind: 'clipboard' } });
			ui.toast('Chat linked from the clipboard');
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	async function override(field: 'model' | 'effort' | 'mode', value: string): Promise<void> {
		if (!v) return;
		const c = await ipc();
		const cur = v.override ?? { model: null, effort: null, mode: null };
		const val = value === '' ? null : value;
		const next: RunOverride = {
			model: field === 'model' ? (val as Model | null) : cur.model,
			effort: field === 'effort' ? (val as Effort | null) : cur.effort,
			mode: field === 'mode' ? (val as Mode | null) : cur.mode
		};
		const empty = !next.model && !next.effort && !next.mode;
		try {
			view.set(
				await c.runOverrideSet({ projectId: pid, step: step.key, over: empty ? null : next })
			);
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	const chipsOn = $state<Record<string, boolean>>({ 'same-chat': true, model: true });
	const argvText = $derived(
		(v?.argvPreview ?? [])
			.map((a, i, all) =>
				i === all.length - 1
					? `"${(a.split('\n')[0] ?? '').slice(0, 60)} …"`
					: /\s/.test(a)
						? `"${a}"`
						: a
			)
			.join(' ')
			.replace(/ (--(?:model|name|settings|agents))/g, '\n  $1')
			.replace(/ ("[^"]*…")$/, '\n  $1')
	);
	const MODELS: Model[] = ['opus', 'sonnet', 'haiku'];
	const EFFORTS: Effort[] = ['low', 'medium', 'high', 'xhigh', 'max'];
	const MODES: Mode[] = ['plan', 'default', 'acceptEdits'];
</script>

<li
	class={[
		'step',
		selected && 'is-selected',
		step.state === 'done' && 'is-done',
		isNa && 'is-na',
		!selected && !quiet && 'is-quiet',
		step.state === 'needsAnnotation' && 'is-annot'
	]}
	aria-current={selected ? 'step' : undefined}
>
	<span class="callout" aria-hidden="true">{step.number}</span>
	<div class="step-head">
		<h2 class="step-title">
			{#if isNa || step.state === 'needsAnnotation'}
				{step.title}
			{:else}
				<button
					type="button"
					aria-expanded={expanded}
					aria-controls="body-{sid}"
					onclick={() => {
						onselect();
						ontoggle();
					}}
					onfocus={onselect}><span class="vh">{step.number} </span>{step.title}</button
				>
			{/if}
		</h2>
		{#if step.tag === 'added'}<span class="badge-added">Added for this project</span>{/if}
		{#if isNa}
			<p class="na-reason">
				{step.whyNotRunnable}{#if step.conditionNote}&nbsp;<span class="muted"
						>({step.conditionNote})</span
					>{/if}
			</p>
		{/if}
		<span class={['step-state', tone]}>
			<Icon name={glyph} size="glyph" tone={look.tone} />{step.state === 'undecided'
				? (step.whyNotRunnable ?? step.stateText)
				: step.state === 'needsAnnotation'
					? 'Not runnable: needs annotation'
					: step.stateText}
		</span>
		{#if step.tag === 'core' && step.state !== 'needsAnnotation'}
			<span class="lock" title={LOCK}
				><Icon name="lock" size="glyph" tone="muted" /><span class="vh">Core step</span></span
			>
		{/if}
	</div>
	{#if step.specLine && !isNa}
		<SpecLine line={step.specLine} id="spec-{sid}" />
	{/if}

	{#if expanded && !isNa}
		<div class="step-body" id="body-{sid}">
			{#if !v}
				<p class="reason" role="status">Loading…</p>
			{:else}
				{#if full && v.chips}
					<div class="chips" role="group" aria-label="Chat setup: check each before you send">
						{#each v.chips.chips as ch (ch.id)}
							<button
								type="button"
								class={['chip', ch.strong && 'strong']}
								role="checkbox"
								aria-checked={chipsOn[ch.id] ?? false}
								onclick={() => (chipsOn[ch.id] = !(chipsOn[ch.id] ?? false))}
								><span class={['box', chipsOn[ch.id] && 'on']} aria-hidden="true"
									>{#if chipsOn[ch.id]}<Icon name="check" size="none" />{/if}</span
								>{ch.label}</button
							>
						{/each}
						<span class="chips-note">Reminders only; Foreman can't set these in claude.ai.</span>
					</div>
				{/if}

				{#if v.inputs.length || v.parts.length}
					<div class="run-inputs">
						{#each v.inputs as inp (inp.name)}
							{@const empty = inp.required && !inp.value}
							<div class="field">
								<label for={inputId(inp.name)}
									>{inp.label}
									<span class="muted"
										>{inp.remember
											? `(run input, remembered for ${project.summary.name})`
											: `(run input for ${inp.token})`}</span
									></label
								>
								<textarea
									class="textarea"
									id={inputId(inp.name)}
									rows="2"
									value={inp.value}
									readonly={!!project.readOnly}
									aria-invalid={empty ? 'true' : undefined}
									aria-describedby={empty || inp.warning ? `${inputId(inp.name)}-msg` : undefined}
									oninput={(e) => {
										setInput(inp.name, e.currentTarget.value);
									}}
									onkeydown={(e) => {
										if (e.metaKey && e.key === 'Enter') {
											e.preventDefault();
											e.stopPropagation();
											void run().then(report);
										}
									}}></textarea>
								{#if empty}
									<p class="err" id="{inputId(inp.name)}-msg">
										<Icon name="cross" size="glyph" />Fill {inp.label} to run this step.
									</p>
								{:else if inp.warning}
									<p class="reason attention" id="{inputId(inp.name)}-msg">
										<Icon name="warn" size="glyph" />{inp.warning}
									</p>
								{/if}
							</div>
						{/each}
						{#each v.parts as part (part.partId)}
							<div class="toggle">
								<button
									type="button"
									class="switch"
									role="switch"
									aria-checked={part.included}
									aria-labelledby="part-{sid}-{part.partId}"
									disabled={part.locked || !!project.readOnly}
									onclick={() => setPart(part.partId, !part.included)}
								></button>
								<span id="part-{sid}-{part.partId}">Include optional part: {part.label}</span>
							</div>
						{/each}
					</div>
				{/if}

				<div class="actions">
					<button
						class="btn primary"
						type="button"
						aria-disabled={blocked || busy ? 'true' : undefined}
						aria-describedby={reason ? `why-${sid}` : undefined}
						onclick={() => {
							if (blocked) ui.say(reason ?? 'This step is blocked.');
							else void run().then(report);
						}}
						><Icon name={primary.icon} />{primary.label}{#if selected}<kbd>⌘↵</kbd>{/if}</button
					>
					{#if full && step.kind !== 'action'}
						<button class="btn" type="button" onclick={() => copy('prompt').then(report)}
							><Icon name="copy" />Copy prompt</button
						>
						{#if step.surface === 'cli'}
							<button class="btn ghost" type="button" onclick={() => copy('command').then(report)}
								>Copy command</button
							>
						{/if}
						{#if v.chips}
							<button
								class="btn ghost"
								type="button"
								onclick={linkChat}
								aria-label={v.chips.linkLabel
									? `Chat link ${v.chips.linkLabel}. Replace it with the link on the clipboard`
									: 'Link this chat from the clipboard'}
								><Icon name="link" />{v.chips.linkLabel ?? 'Link this chat'}</button
							>
						{/if}
						{#if step.surface === 'chat' && !writesPacket && step.state === 'copied'}
							<button class="btn ghost" type="button" onclick={() => markDone().then(report)}
								>Mark done<kbd>⌘D</kbd></button
							>
						{/if}
					{/if}
					{#if (step.tag === 'optional' || step.tag === 'added') && step.state !== 'done' && !project.readOnly && full}
						<button
							class="btn ghost"
							type="button"
							aria-expanded={skipping}
							onclick={() => (skipping = !skipping)}><Icon name="skip" />Skip</button
						>
					{/if}
					{#if reason}<span class="reason" id="why-{sid}">{reason}</span>{/if}
				</div>

				{#if skipping}
					<form
						class="skip"
						onsubmit={(e) => {
							e.preventDefault();
							void skip();
						}}
					>
						<div class="field">
							<label for="skip-{sid}"
								>Why skip {step.number}? One line, recorded under SKIPPED at signing.</label
							>
							<div class="input-row">
								<input
									class="input"
									id="skip-{sid}"
									bind:value={skipReason}
									maxlength="200"
									aria-invalid={skipError ? 'true' : undefined}
									aria-describedby={skipError ? `skip-${sid}-err` : undefined}
								/>
								<button class="btn" type="submit">Skip step</button>
							</div>
							{#if skipError}<p class="err" id="skip-{sid}-err">
									<Icon name="cross" size="glyph" />{skipError}
								</p>{/if}
						</div>
					</form>
				{/if}

				{#if full && step.surface === 'chat' && step.state !== 'done'}
					<PasteBack
						api={(t: (() => Promise<true | string>) | null) => (pasteTrigger = t)}
						projectId={pid}
						step={step.key}
						stage={stage.stage}
						packet={writesPacket}
					/>
				{/if}

				{#if full && v.prompt.length}
					<PromptBlock segments={v.prompt} stepNumber={step.number} inputs={v.inputs} {inputId} />
				{/if}
				{#each v.remaps as r, i (i)}<p class="remap">{r}</p>{/each}

				{#if full && v.argvPreview}
					<details class="argv">
						<summary><Icon name="chev" class="chev" /><span>Show the exact command</span></summary>
						<pre>{argvText}</pre>
						<p class="remap env">
							Environment names (values stay in Rust): {v.envNames.join(', ')}
						</p>
					</details>
					{#if v.start}
						<details class="argv">
							<summary
								><Icon name="chev" class="chev" /><span
									>Change this run's model, effort or mode</span
								></summary
							>
							<div class="override">
								<label
									>Model <select
										class="select"
										value={v.override?.model ?? ''}
										onchange={(e) => override('model', e.currentTarget.value)}
									>
										<option value="">Template ({MODEL_NAME[v.start.value.model]})</option>
										{#each MODELS as m (m)}<option value={m}>{MODEL_NAME[m]}</option>{/each}
									</select></label
								>
								<label
									>Effort <select
										class="select"
										value={v.override?.effort ?? ''}
										onchange={(e) => override('effort', e.currentTarget.value)}
									>
										<option value="">Template ({v.start.value.effort})</option>
										{#each EFFORTS as m (m)}<option value={m}>{m}</option>{/each}
									</select></label
								>
								<label
									>Mode <select
										class="select"
										value={v.override?.mode ?? ''}
										onchange={(e) => override('mode', e.currentTarget.value)}
									>
										<option value="">Template ({MODE_NAME[v.start.value.mode ?? 'default']})</option
										>
										{#each MODES as m (m)}<option value={m}>{MODE_NAME[m]}</option>{/each}
									</select></label
								>
							</div>
							<p class="remap">
								A different mode or model asks you once more in a macOS dialog before the launch.
							</p>
						</details>
					{/if}
				{/if}
				{#if full}
					{#each v.warnings as w, i (i)}<p class="reason attention">
							<Icon name="warn" size="glyph" /><Text text={w} />
						</p>{/each}
				{/if}
				{#if full && step.tag === 'core'}
					<p class="step-note"><Icon name="lock" size="glyph" tone="muted" />{LOCK}</p>
				{/if}
			{/if}
		</div>
	{/if}
</li>

{#if action}
	<Dialog
		title="Write {action.writes.length} files for {step.number} {step.title}"
		description="Foreman writes only these paths and never overwrites a newer file."
		onclose={() => (action = null)}
	>
		{#each action.writes as d (d.path)}
			<Diff diff={d} label="New file {d.path}" />
		{/each}
		{#snippet footer()}
			<span class="spacer"></span>
			<button class="btn" type="button" onclick={() => (action = null)}>Cancel</button>
			<button class="btn primary" type="button" onclick={applyAction}
				>Write {action?.writes.length} files</button
			>
		{/snippet}
	</Dialog>
{/if}

<style>
	.step {
		display: grid;
		grid-template-columns: var(--step-margin) minmax(0, 1fr);
		padding: var(--space-4) var(--space-4) var(--space-4) 0;
		border-bottom: var(--hairline) solid var(--rule);
	}
	.step.is-selected {
		background: var(--sheet-raised);
	}
	.step.is-done,
	.step.is-na,
	.step.is-quiet {
		padding-top: 9px;
		padding-bottom: 9px;
	}
	.step.is-done .callout,
	.step.is-na .callout,
	.step.is-quiet .callout {
		width: 30px;
		height: 30px;
		margin-top: -4px;
		font-size: var(--text-11);
	}
	.step.is-done :global(.spec-line) {
		display: none;
	}
	.callout {
		grid-row: 1 / span 3;
		justify-self: center;
		align-self: start;
		position: relative;
		width: 34px;
		height: 34px;
		border-radius: 50%;
		border: var(--hairline) solid var(--ink-muted);
		display: grid;
		place-items: center;
		font-size: var(--text-12);
		font-weight: var(--weight-semibold);
		color: var(--ink);
		margin-top: -6px;
		white-space: nowrap;
	}
	.callout::after {
		content: '';
		position: absolute;
		left: 100%;
		top: 50%;
		width: 15px;
		height: 1px;
		background: var(--ink-muted);
	}
	.step.is-selected .callout {
		border-color: var(--accent-ink);
		color: var(--accent-ink);
		border-width: 1.5px;
	}
	.step.is-selected .callout::after {
		background: var(--accent-ink);
	}
	.step.is-done .callout {
		color: var(--ink-muted);
	}
	.step.is-annot .callout {
		border-style: dashed;
		color: var(--ink-muted);
	}
	.step-head {
		display: flex;
		align-items: baseline;
		gap: var(--space-4);
		min-width: 0;
	}
	.step-title {
		font-size: var(--text-15);
		font-weight: var(--weight-semibold);
		line-height: 1.3;
		min-width: 0;
	}
	.step-title button {
		all: unset;
		cursor: default;
	}
	.step-title button:focus-visible {
		outline: var(--focus-width) solid var(--focus);
		outline-offset: var(--focus-offset);
	}
	.step.is-done .step-title,
	.step.is-na .step-title,
	.step.is-quiet .step-title {
		font-weight: var(--weight-medium);
	}
	.step-state {
		margin-left: auto;
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: var(--text-12);
		color: var(--ink-muted);
		white-space: nowrap;
	}
	.step-state.pass {
		color: var(--pass);
	}
	.step-state.attention {
		color: var(--attention);
	}
	.step-state.accent {
		color: var(--accent-ink);
	}
	.lock {
		display: inline-flex;
		color: var(--ink-muted);
	}
	.badge-added {
		font-size: var(--text-11);
		color: var(--accent-ink);
		border: 1px solid var(--accent-ink);
		border-radius: var(--radius-pill);
		padding: 0 5px;
		font-weight: var(--weight-medium);
		white-space: nowrap;
	}
	.step.is-na {
		background-image: repeating-linear-gradient(135deg, var(--hatch) 0 1px, transparent 1px 7px);
	}
	.step.is-na .callout {
		border-style: dashed;
		color: var(--ink-muted);
		background: var(--sheet);
	}
	.na-reason {
		color: var(--ink);
		font-size: var(--text-12);
		min-width: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.step-body {
		margin-top: var(--space-4);
		display: grid;
		gap: var(--space-4);
		max-width: calc(var(--measure-prompt) + 40px);
	}
	.step-note {
		font-size: var(--text-12);
		color: var(--ink-muted);
		display: flex;
		gap: 6px;
		align-items: center;
	}
	.run-inputs {
		display: grid;
		gap: var(--space-4);
		max-width: var(--measure-prompt);
	}
	.env {
		margin-top: 6px;
	}
	.override {
		display: flex;
		gap: var(--space-4);
		margin-top: var(--space-3);
		font-size: var(--text-12);
	}
	.override label {
		display: grid;
		gap: var(--space-2);
	}
	.skip {
		max-width: 560px;
	}
	@media (prefers-contrast: more) {
		.step.is-na {
			background-image: none;
		}
	}
</style>
