<script lang="ts">
	// The + between two steps (F-025, P2, mockup workspace.html): add a prompt or an inspection item
	// for this project only. A prompt starts from the run config of a step Billy picks, lets him
	// edit the text and the config, and is optional unless he makes it core; it is stored in
	// overlay.json, shows "Added for this project" and always confirms before running (SR-19).
	// An item is an attestation. Rust validates everything again (DR-061).
	import { tick } from 'svelte';

	import { failureText } from '#lib/actions/words.ts';
	import { MODE_NAME, MODEL_NAME } from '#lib/actions/words.ts';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type {
		CwdKind,
		Effort,
		Mode,
		Model,
		ProjectView,
		StageView,
		StepSummary,
		Surface
	} from '#lib/ipc/types.ts';
	import { refresh } from '#lib/stores/bus.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import Dialog from './Dialog.svelte';
	import Icon from './Icon.svelte';
	import Segmented from './Segmented.svelte';

	let { project, stage, after }: { project: ProjectView; stage: StageView; after: StepSummary } =
		$props();

	const MODELS: Model[] = ['fable', 'opus', 'sonnet', 'haiku', 'opusplan'];
	const EFFORTS: Effort[] = ['low', 'medium', 'high', 'xhigh', 'max'];
	const MODES: Mode[] = ['plan', 'default', 'acceptEdits'];
	const SURFACES: { value: Surface; label: string }[] = [
		{ value: 'cli', label: 'Terminal' },
		{ value: 'chat', label: 'Chat' },
		{ value: 'desktop', label: 'Desktop' }
	];

	let menuOpen = $state(false);
	let button = $state<HTMLButtonElement>();
	let menu = $state<HTMLElement>();
	let adding = $state<'prompt' | 'item' | null>(null);

	// The prompt form.
	let title = $state('');
	let text = $state('');
	let copyFrom = $state('');
	let surface = $state<Surface>('cli');
	let cwd = $state<CwdKind>('spec');
	let model = $state<Model>('opus');
	let effort = $state<Effort>('high');
	let mode = $state<Mode>('plan');
	let outs = $state('');
	let core = $state(false);
	let error = $state<string | null>(null);
	let busy = $state(false);

	/** Steps whose run config can be copied: the ones that run something. */
	const sources = $derived(
		stage.steps.filter((s) => s.surface !== null && s.kind !== 'action' && s.kind !== 'manual')
	);
	const label = $derived(`Add to Stage ${stage.stage} after ${after.number}`);

	/** Loads a step's run config into the form (the "copies the run config of a step he picks" rule). */
	async function load(key: string): Promise<void> {
		const step = stage.steps.find((s) => s.key === key);
		if (step?.surface) surface = step.surface;
		const where = step?.specLine?.where ?? '';
		cwd = where.startsWith('spec') ? 'spec' : 'project';
		try {
			const c = await ipc();
			const v = await c.stepView({ projectId: project.summary.id, step: key });
			const s = v.start?.value;
			if (s) {
				model = s.model;
				effort = s.effort;
				mode = s.mode ?? 'plan';
			}
		} catch {
			// The form keeps its values; Rust refuses an invalid config with a sentence.
		}
	}

	async function openForm(kind: 'prompt' | 'item'): Promise<void> {
		menuOpen = false;
		error = null;
		title = '';
		text = '';
		outs = '';
		core = false;
		adding = kind;
		if (kind === 'prompt') {
			const start = sources.find((s) => s.key === after.key) ?? sources[0];
			copyFrom = start?.key ?? after.key;
			await load(copyFrom);
		}
	}

	function onSurface(s: Surface): void {
		surface = s;
		if (s === 'chat') effort = 'high';
	}

	async function submit(): Promise<void> {
		if (busy) return;
		busy = true;
		error = null;
		try {
			const c = await ipc();
			if (adding === 'prompt') {
				const chat = surface === 'chat';
				await c.addStep({
					projectId: project.summary.id,
					stage: stage.stage,
					after: after.key,
					copyConfigFrom: copyFrom,
					title: title.trim(),
					text,
					config: {
						surface,
						cwd: chat ? null : cwd,
						start: {
							model,
							effort: chat ? 'adaptive' : effort,
							mode: chat ? null : mode
						},
						out: outs
							.split('\n')
							.map((l) => l.trim())
							.filter(Boolean)
					},
					tag: core ? 'core' : 'optional'
				});
				ui.toast('Prompt added for this project');
			} else {
				await c.addItem({ projectId: project.summary.id, stage: stage.stage, text: text.trim() });
				ui.toast('Inspection item added for this project');
			}
			adding = null;
			refresh(`project:${project.summary.id}`);
		} catch (e: unknown) {
			error = failureText(toIpcFailure(e));
		} finally {
			busy = false;
		}
	}

	function items(): HTMLElement[] {
		return menu ? [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')] : [];
	}
	async function toggle(): Promise<void> {
		menuOpen = !menuOpen;
		if (menuOpen) {
			await tick();
			items()[0]?.focus();
		}
	}
	function onMenuKey(e: KeyboardEvent): void {
		const els = items();
		const i = els.findIndex((el) => el === document.activeElement);
		const go = (n: number): void => {
			e.preventDefault();
			els[(n + els.length) % els.length]?.focus();
		};
		if (e.key === 'ArrowDown') go(i + 1);
		else if (e.key === 'ArrowUp') go(i - 1);
		else if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			menuOpen = false;
			button?.focus();
		} else if (e.key === 'Tab') menuOpen = false;
	}
	const valid = $derived(
		adding === 'prompt' ? !!title.trim() && !!text.trim() : !!text.trim() && !text.includes('\n')
	);
</script>

<li class="insert">
	<button
		type="button"
		bind:this={button}
		aria-label="{label}: prompt or inspection item, this project only"
		aria-haspopup="menu"
		aria-expanded={menuOpen}
		title="Add prompt or inspection item (this project only)"
		onclick={toggle}><Icon name="plus" size="none" /></button
	>
	{#if menuOpen}
		<div
			class="menu"
			role="menu"
			aria-label="Add to this stage"
			tabindex="-1"
			bind:this={menu}
			onkeydown={onMenuKey}
		>
			<button role="menuitem" type="button" tabindex="-1" onclick={() => openForm('prompt')}
				>Add prompt <span class="muted">this project only</span></button
			>
			<button role="menuitem" type="button" tabindex="-1" onclick={() => openForm('item')}
				>Add inspection item <span class="muted">this project only</span></button
			>
		</div>
	{/if}
</li>

{#if adding}
	<Dialog
		title={adding === 'prompt' ? `Add a prompt after ${after.number}` : 'Add an inspection item'}
		description="For this project only. It is written to .foreman/overlay.json, and an added prompt always asks before it runs."
		onclose={() => (adding = null)}
		width="narrow"
	>
		{#if adding === 'prompt'}
			<div class="form">
				<div class="field">
					<label for="add-title">Title</label>
					<input class="input" id="add-title" bind:value={title} maxlength="200" />
				</div>
				<div class="field">
					<label for="add-text">Prompt</label>
					<textarea
						class="textarea"
						id="add-text"
						rows="5"
						maxlength="4000"
						bind:value={text}
						aria-invalid={error ? 'true' : undefined}
						aria-describedby={error ? 'add-err' : undefined}></textarea>
				</div>
				<div class="field">
					<label for="add-copy">Start from the run config of</label>
					<select
						class="select"
						id="add-copy"
						bind:value={copyFrom}
						onchange={() => load(copyFrom)}
					>
						{#each sources as s (s.key)}<option value={s.key}>{s.number} {s.title}</option>{/each}
					</select>
				</div>
				<div class="field">
					<span id="add-surface">Runs in</span>
					<Segmented
						options={SURFACES}
						value={surface}
						labelledby="add-surface"
						onchange={onSurface}
					/>
				</div>
				<div class="row2">
					{#if surface !== 'chat'}
						<div class="field">
							<label for="add-cwd">Folder</label>
							<select class="select" id="add-cwd" bind:value={cwd}>
								<option value="spec">spec/</option>
								<option value="project">Project root</option>
							</select>
						</div>
						<div class="field">
							<label for="add-mode">Mode</label>
							<select class="select" id="add-mode" bind:value={mode}>
								{#each MODES as m (m)}<option value={m}>{MODE_NAME[m]}</option>{/each}
							</select>
						</div>
					{/if}
					<div class="field">
						<label for="add-model">Model</label>
						<select class="select" id="add-model" bind:value={model}>
							{#each MODELS as m (m)}<option value={m}>{MODEL_NAME[m]}</option>{/each}
						</select>
					</div>
					{#if surface !== 'chat'}
						<div class="field">
							<label for="add-effort">Effort</label>
							<select class="select" id="add-effort" bind:value={effort}>
								{#each EFFORTS as m (m)}<option value={m}>{m}</option>{/each}
							</select>
						</div>
					{:else}
						<p class="hint">Chat uses adaptive effort and has no mode.</p>
					{/if}
				</div>
				<div class="field">
					<label for="add-out"
						>Files it writes <span class="muted">(optional, one per line)</span></label
					>
					<textarea class="textarea" id="add-out" rows="2" bind:value={outs} placeholder="docs/…"
					></textarea>
				</div>
				<label class="toggle">
					<input type="checkbox" bind:checked={core} />
					<span
						>Make it core for this project <span class="muted">(then it can't be skipped)</span
						></span
					>
				</label>
				{#if error}<p class="err" id="add-err" role="alert">
						<Icon name="cross" size="glyph" />{error}
					</p>{/if}
			</div>
		{:else}
			<div class="field">
				<label for="add-text">Inspection item</label>
				<input
					class="input"
					id="add-text"
					bind:value={text}
					maxlength="200"
					aria-invalid={error ? 'true' : undefined}
					aria-describedby="add-help{error ? ' add-err' : ''}"
				/>
				<p class="hint" id="add-help">
					You attest it at the gate with a note, like any item. It appears in the Gate A list.
				</p>
				{#if error}<p class="err" id="add-err" role="alert">
						<Icon name="cross" size="glyph" />{error}
					</p>{/if}
			</div>
		{/if}
		{#snippet footer()}
			<span class="spacer"></span>
			<button class="btn" type="button" onclick={() => (adding = null)}>Cancel</button>
			<button
				class="btn primary"
				type="button"
				aria-disabled={!valid || busy ? 'true' : undefined}
				onclick={() => (valid ? submit() : ui.say('Fill in the title and the text first.'))}
				>Add</button
			>
		{/snippet}
	</Dialog>
{/if}

<style>
	.insert {
		display: grid;
		grid-template-columns: var(--step-margin) 1fr;
		align-items: center;
		height: 0;
		position: relative;
		z-index: 1;
	}
	.insert > button {
		justify-self: center;
		width: 18px;
		height: 18px;
		margin-top: -1px;
		border-radius: 50%;
		border: 1px solid var(--control-edge);
		background: var(--sheet);
		color: var(--ink-muted);
		display: grid;
		place-items: center;
		padding: 0;
		transform: translateY(-50%);
	}
	.insert > button:hover,
	.insert > button:focus-visible {
		color: var(--ink);
	}
	.insert :global(svg) {
		width: 10px;
		height: 10px;
		stroke: currentColor;
		stroke-width: 1.6;
		fill: none;
	}
	.menu {
		position: absolute;
		left: var(--step-margin);
		top: 100%;
		margin-top: var(--space-3);
		z-index: 25;
		display: grid;
		min-width: 260px;
		padding: var(--space-2);
		background: var(--sheet-raised);
		border-radius: var(--radius-control);
		box-shadow: var(--shadow-overlay);
	}
	.menu button {
		text-align: left;
		border: 0;
		background: transparent;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-control);
	}
	.menu button:hover,
	.menu button:focus-visible {
		background: var(--selected);
	}
	.form {
		display: grid;
		gap: var(--space-4);
	}
	.row2 {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--space-4);
		align-items: end;
	}
	.toggle {
		display: flex;
		gap: var(--space-3);
		align-items: center;
		font-size: var(--text-13);
	}
</style>
