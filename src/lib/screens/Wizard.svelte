<script lang="ts">
	// New project wizard (F-018 P2, mockup wizard.html): the setup form's fields in four steps, ending
	// on "Review your path": which stages apply and why, the strip preview and the exact writes.
	import { goto } from '$app/navigation';
	import { onMount, tick, untrack } from 'svelte';

	import AnnotationProblems from '#lib/components/AnnotationProblems.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import type { IconName } from '#lib/components/icons.ts';
	import Segmented from '#lib/components/Segmented.svelte';
	import SheetStrip from '#lib/components/SheetStrip.svelte';
	import type { ApplicablePathRow, TriState } from '#lib/ipc/types.ts';
	import { registerShortcuts } from '#lib/keyboard/registry.ts';
	import { to } from '#lib/nav/routes.ts';
	import Text from '#lib/render/Text.svelte';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import { SetupForm } from './setup/form.svelte.ts';

	let { start = 0, form = null }: { start?: number; form?: SetupForm | null } = $props();
	const f = untrack(() => form) ?? new SetupForm();
	let at = $state(untrack(() => start));
	const STEPS = ['Name and folder', 'Variables', 'Conditions', 'Review your path'];
	$effect(() => {
		ui.setChrome({ title: 'New project', sub: `Step ${at + 1} of 4` });
	});

	const TRI: { value: TriState; label: string }[] = [
		{ value: 'yes', label: 'Yes' },
		{ value: 'no', label: 'No' },
		{ value: 'undecided', label: 'Undecided' }
	];
	const errors = $derived(f.errors);
	const stepErrors = $derived(
		at === 0
			? ['name', 'folder', 'url'].filter((k) => errors[k])
			: at === 1
				? Object.keys(errors).filter((k) => k.startsWith('var-'))
				: []
	);
	const show = (k: string) => (f.touched[k] ?? false) && !!errors[k];

	async function next(): Promise<void> {
		if (stepErrors.length) {
			for (const k of stepErrors) f.touched[k] = true;
			ui.say('Fix the highlighted fields first.');
			return;
		}
		if (at < 3) {
			at += 1;
			if (at === 3) await f.refreshPreview();
			await tick();
			document.getElementById('wiz-title')?.focus();
			return;
		}
		const p = await f.create();
		if (p) {
			ui.toast(`Project created: ${p.name}`);
			await goto(to.project(p.id));
		} else ui.say(f.createError ?? 'Fix the fields first.');
	}

	onMount(() => {
		void f.load();
		if (start === 0) document.getElementById('w-name')?.focus();
		return registerShortcuts({
			run: async () => {
				await next();
				return true as const;
			}
		});
	});

	const look = (r: ApplicablePathRow): { icon: IconName; tone: 'pass' | 'attention' | 'muted' } =>
		r.status === 'applies'
			? { icon: 'check', tone: 'pass' }
			: r.status === 'undecided'
				? { icon: 'question', tone: 'attention' }
				: r.status === 'notApplicable'
					? { icon: 'hatch', tone: 'muted' }
					: { icon: 'dashsq', tone: 'muted' };
	const STAGE_TITLES = [
		'Land Survey',
		'Meet the Architect',
		"Architect's Design Studio",
		'Permits & Zoning',
		'Construction Drawings',
		'Site Prep & Excavation',
		'Foundation',
		'Framing',
		'Roofing',
		'Plumbing',
		'Electrical',
		'HVAC',
		'Insulation & Drywall',
		'Siding & Exterior',
		'Inspections',
		'Paint & Trim',
		'Final Walkthrough',
		'Certificate of Occupancy',
		'Homeowner Maintenance'
	];
	const writes = $derived(f.preview?.writes ?? []);
</script>

<main class="main" id="main" aria-labelledby="wiz-title">
	<div class="wiz">
		<nav aria-label="Wizard steps">
			<ol class="wiz-steps">
				{#each STEPS as s, i (s)}
					<li class={[i < at && 'done']} aria-current={i === at ? 'step' : undefined}>
						{#if i < at}<Icon name="check" size="glyph" tone="pass" />{:else}<span
								class="n"
								aria-hidden="true">{i + 1}</span
							>{/if}
						<span
							>{s}{#if i < at}<span class="vh">, done</span>{/if}</span
						>
					</li>
				{/each}
			</ol>
		</nav>
		<div>
			{#if at === 0}
				<h1 class="page-title" id="wiz-title" tabindex="-1">Name and folder</h1>
				<p class="page-sub">
					The product's name and the folder that holds it. Foreman adds .foreman/ and the docs it
					needs.
				</p>
				<div class="fields">
					<div class="field">
						<label for="w-name">Name</label>
						<input
							class="input"
							id="w-name"
							bind:value={f.name}
							onblur={() => (f.touched.name = true)}
							aria-invalid={show('name') ? 'true' : undefined}
							aria-describedby={show('name') ? 'w-name-err' : undefined}
						/>
						{#if show('name')}<p class="err" id="w-name-err">
								<Icon name="cross" size="glyph" />{errors.name}
							</p>{/if}
					</div>
					<div class="field">
						<label for="w-folder">Folder</label>
						<div class="input-row">
							<input
								class="input mono"
								id="w-folder"
								readonly
								value={f.pick?.displayPath ?? ''}
								placeholder="Choose a folder"
								aria-invalid={show('folder') ? 'true' : undefined}
								aria-describedby={show('folder') ? 'w-folder-err' : undefined}
							/>
							<button class="btn" type="button" onclick={() => f.choose()}>Choose…</button>
						</div>
						{#if show('folder')}<p class="err" id="w-folder-err">
								<Icon name="cross" size="glyph" /><span><Text text={errors.folder ?? ''} /></span>
							</p>{/if}
					</div>
					<div class="field">
						<label for="w-url">claude.ai Project link</label>
						<input
							class="input mono"
							id="w-url"
							bind:value={f.url}
							placeholder="https://claude.ai/project/…"
							onblur={() => (f.touched.url = true)}
							aria-invalid={show('url') ? 'true' : undefined}
							aria-describedby={show('url') ? 'w-url-err' : undefined}
						/>
						{#if show('url')}<p class="err" id="w-url-err">
								<Icon name="cross" size="glyph" />{errors.url}
							</p>{/if}
					</div>
				</div>
			{:else if at === 1}
				<h1 class="page-title" id="wiz-title" tabindex="-1">Variables</h1>
				<p class="page-sub">Prompts use these words. A blueprint placeholder counts as empty.</p>
				<div class="fields">
					{#each f.specs as v (v.name)}
						{@const k = `var-${v.name}`}
						<div class="field">
							<label for="wv-{v.name}">{v.label} <code>[{v.name}]</code></label>
							<input
								class="input"
								id="wv-{v.name}"
								bind:value={f.variables[v.name]}
								placeholder={v.placeholder}
								onblur={() => (f.touched[k] = true)}
								aria-invalid={show(k) ? 'true' : undefined}
								aria-describedby={show(k) ? `wv-${v.name}-err` : undefined}
							/>
							{#if show(k)}<p class="err" id="wv-{v.name}-err">
									<Icon name="cross" size="glyph" />{errors[k]}
								</p>{:else if v.fromSettings && f.defaults[v.name]}<p class="hint">
									From your defaults in Settings
								</p>{/if}
						</div>
					{/each}
				</div>
			{:else if at === 2}
				<h1 class="page-title" id="wiz-title" tabindex="-1">Conditions</h1>
				<p class="page-sub">
					Each one decides which steps apply. Undecided blocks the Stage 2 sign.
				</p>
				<div class="fields">
					{#each f.conditionSpecs as c (c.id)}
						<div class="cond">
							<span id="wc-{c.id}"><b>{c.label}</b> <code>{c.id}</code></span>
							<p>{c.effect}</p>
							<Segmented
								options={TRI}
								value={f.conditions[c.id] ?? 'undecided'}
								labelledby="wc-{c.id}"
								onchange={(v: TriState) => (f.conditions[c.id] = v)}
							/>
						</div>
					{/each}
				</div>
			{:else}
				<h1 class="page-title" id="wiz-title" tabindex="-1">
					Review {f.name || 'the project'}'s path
				</h1>
				{#if f.preview}
					<p class="page-sub">
						{f.preview.path.applies} of 19 stages apply. Stages that don't apply say why; change a condition
						to bring one back before Stage 2 is signed.
					</p>
					<div class="path-strip">
						<SheetStrip
							projectId="preview"
							projectName={f.name || 'New project'}
							stages={f.preview.path.strip.map((s) => ({
								...s,
								title: STAGE_TITLES[s.stage] ?? '',
								statusWhy: s.why
							}))}
							open={null}
							preview
						/>
					</div>
					<ol class="path" aria-label="Stages and whether they apply">
						{#each f.preview.path.rows as r (r.stages)}
							{@const l = look(r)}
							<li class={[r.status === 'notApplicable' && 'is-na']}>
								<span class="num">{r.stages}</span><Icon
									name={l.icon}
									size="glyph"
									tone={l.tone}
								/><span>{r.title}</span><span class="why">{r.why}</span>
							</li>
						{/each}
					</ol>
					<div class="writes-inline" aria-label="File writes">
						{#each [{ h: 'Create', a: 'create' }, { h: 'Leave alone', a: 'leaveAlone' }, { h: 'Refuse to overwrite', a: 'refuse' }] as g (g.a)}
							{@const list = writes.filter((w) => w.action === g.a)}
							<div>
								<h2 class="wi-h">{g.h}</h2>
								<ul>
									{#each list as w (w.path)}<li>
											<code><Text text={w.path} layout={false} /></code>{#if g.a === 'leaveAlone'},
												exists{/if}
										</li>{:else}<li>None</li>{/each}
								</ul>
							</div>
						{/each}
					</div>
				{:else if f.previewProblems.length}
					<AnnotationProblems problems={f.previewProblems} />
				{:else if f.previewError}
					<p class="err" role="alert"><Icon name="cross" size="glyph" />{f.previewError}</p>
				{/if}
			{/if}
			<div class="wiz-foot">
				{#if at > 0}<button class="btn" type="button" onclick={() => (at -= 1)}>Back</button>{/if}
				<span class="spacer"></span>
				{#if at === 3 && f.preview}
					<span class="reason"
						>Creates {writes.filter((w) => w.action === 'create').length} paths and pins blueprint {f
							.preview.pin.version} at {f.preview.pin.commit}</span
					>
				{/if}
				<button
					class="btn primary lg"
					type="button"
					aria-disabled={at === 3 && !f.valid ? 'true' : undefined}
					onclick={next}>{at === 3 ? 'Create project' : 'Continue'}<kbd>⌘↵</kbd></button
				>
			</div>
		</div>
	</div>
</main>

<style>
	.wiz {
		display: grid;
		grid-template-columns: 200px minmax(0, 760px);
		gap: var(--space-7);
		padding: var(--space-6) var(--space-7) var(--space-8);
	}
	.wiz-steps {
		display: grid;
		gap: var(--space-1);
		align-content: start;
	}
	.wiz-steps li {
		display: grid;
		grid-template-columns: 22px 1fr;
		gap: var(--space-3);
		align-items: center;
		padding: 6px var(--space-3);
		border-radius: var(--radius-control);
		color: var(--ink-muted);
	}
	.wiz-steps li[aria-current] {
		background: var(--sheet-raised);
		color: var(--ink);
		font-weight: var(--weight-medium);
	}
	.n {
		width: 20px;
		height: 20px;
		border-radius: 50%;
		border: 1px solid var(--control-edge);
		display: grid;
		place-items: center;
		font-size: var(--text-11);
	}
	.wiz-steps li[aria-current] .n {
		border-color: var(--accent-ink);
		color: var(--accent-ink);
	}
	.fields {
		display: grid;
		gap: var(--space-4);
		margin-top: var(--space-5);
		max-width: 620px;
	}
	.cond {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: var(--space-1) var(--space-5);
		align-items: center;
		padding: 6px 0;
		border-bottom: var(--hairline) solid var(--rule);
	}
	.cond p {
		grid-column: 1;
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.cond :global(.seg) {
		grid-row: 1 / span 2;
		grid-column: 2;
	}
	.cond code {
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.path-strip {
		margin-top: var(--space-5);
	}
	.path {
		margin-top: var(--space-5);
	}
	.path li {
		display: grid;
		grid-template-columns: 40px 22px 230px minmax(0, 1fr);
		gap: var(--space-3);
		align-items: baseline;
		padding: 6px 0;
		border-bottom: var(--hairline) solid var(--rule);
		font-size: var(--text-13);
	}
	.path li .why {
		color: var(--ink-muted);
		font-size: var(--text-12);
	}
	.path li.is-na {
		background-image: repeating-linear-gradient(135deg, var(--hatch) 0 1px, transparent 1px 7px);
	}
	.num {
		white-space: nowrap;
		text-align: right;
		color: var(--ink-muted);
		font-variant-numeric: tabular-nums;
	}
	.wiz-foot {
		display: flex;
		gap: var(--space-3);
		margin-top: var(--space-6);
		align-items: center;
	}
	.wiz-foot .spacer {
		flex: 1;
	}
	.writes-inline {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--space-5);
		margin-top: var(--space-4);
		font-size: var(--text-12);
	}
	.wi-h {
		font-size: var(--text-12);
		font-weight: var(--weight-semibold);
		color: var(--ink-muted);
		margin-bottom: var(--space-2);
	}
	.writes-inline code {
		font-size: var(--text-12);
	}
	.writes-inline li {
		padding: var(--space-1) 0;
	}
</style>
