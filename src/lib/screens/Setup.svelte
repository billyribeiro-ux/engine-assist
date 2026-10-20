<script lang="ts">
	// New project, one form (F-018 P1, mockup setup.html): name and folder, the claude.ai Project
	// link, variables (placeholders count as empty), the eight early conditions as tri-state
	// radiogroups, and a sticky "What Foreman will write" panel. Create stays disabled and names the
	// fields to fix; ⌘↵ submits when valid (AC-F018-01).
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';

	import Icon from '#lib/components/Icon.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import type { TriState } from '#lib/ipc/types.ts';
	import { registerShortcuts } from '#lib/keyboard/registry.ts';
	import { to } from '#lib/nav/routes.ts';
	import Text from '#lib/render/Text.svelte';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import { SetupForm } from './setup/form.svelte.ts';

	ui.setChrome({ title: 'New project', sub: 'One form; the wizard replaces it in P2' });

	const f = new SetupForm();
	const TRI: { value: TriState; label: string }[] = [
		{ value: 'yes', label: 'Yes' },
		{ value: 'no', label: 'No' },
		{ value: 'undecided', label: 'Undecided' }
	];
	const errors = $derived(f.errors);
	const show = (k: string) => (f.touched[k] ?? false) && !!errors[k];

	async function submit(): Promise<true | string> {
		for (const k of Object.keys(errors)) f.touched[k] = true;
		if (!f.valid)
			return `Fix ${f.fieldsToFix.length} ${f.fieldsToFix.length === 1 ? 'field' : 'fields'} first: ${f.fieldsToFix.join(', ')}.`;
		const p = await f.create();
		if (!p) return f.createError ?? 'The project could not be created.';
		ui.toast(`Project created: ${p.name}`);
		await goto(to.project(p.id));
		return true;
	}

	onMount(() => {
		void f.load();
		return registerShortcuts({ run: () => submit() });
	});

	const groups = $derived({
		create: f.preview?.writes.filter((w) => w.action === 'create') ?? [],
		leave: f.preview?.writes.filter((w) => w.action === 'leaveAlone') ?? [],
		refuse: f.preview?.writes.filter((w) => w.action === 'refuse') ?? []
	});
	const createWhy = $derived(
		f.fieldsToFix.length
			? `Fix ${f.fieldsToFix.length} ${f.fieldsToFix.length === 1 ? 'field' : 'fields'} first: ${f.fieldsToFix.join(', ')}.`
			: !f.preview
				? 'Choose the folder to see what Foreman will write.'
				: null
	);
</script>

<main class="main" id="main" aria-labelledby="setup-title">
	<form
		class="setup"
		aria-labelledby="setup-title"
		novalidate
		onsubmit={(e) => {
			e.preventDefault();
			void submit().then((r) => (r === true ? undefined : ui.say(r)));
		}}
	>
		<div>
			<h1 class="page-title" id="setup-title">New project</h1>
			<p class="page-sub">
				Foreman pins the blueprint, tailors the stages to your answers, and creates the files below.
				Nothing is written until you press Create project.
			</p>

			<div class="form-sec">
				<h2>Name and folder</h2>
				<div class="field">
					<label for="f-name">Name</label>
					<input
						class="input"
						id="f-name"
						bind:value={f.name}
						oninput={() => f.schedule()}
						onblur={() => (f.touched.name = true)}
						aria-invalid={show('name') ? 'true' : undefined}
						aria-describedby={show('name') ? 'f-name-err' : undefined}
					/>
					{#if show('name')}<p class="err" id="f-name-err">
							<Icon name="cross" size="glyph" /><span>{errors.name}</span>
						</p>{/if}
				</div>
				<div class="field">
					<label for="f-folder">Folder</label>
					<div class="input-row">
						<input
							class="input mono"
							id="f-folder"
							readonly
							value={f.pick?.displayPath ?? ''}
							placeholder="Choose a folder"
							aria-invalid={show('folder') ? 'true' : undefined}
							aria-describedby={show('folder') ? 'f-folder-err' : 'f-folder-hint'}
						/>
						<button class="btn" type="button" onclick={() => f.choose()}>Choose…</button>
					</div>
					{#if show('folder')}
						<p class="err" id="f-folder-err">
							<Icon name="cross" size="glyph" /><span><Text text={errors.folder ?? ''} /></span>
						</p>
					{:else}
						<p class="hint" id="f-folder-hint">
							A new or existing folder. macOS asks which one; Foreman never takes a typed path.
						</p>
					{/if}
					{#if f.pickError}<p class="err"><Icon name="cross" size="glyph" />{f.pickError}</p>{/if}
				</div>
				<div class="field">
					<label for="f-url">claude.ai Project link</label>
					<input
						class="input mono"
						id="f-url"
						bind:value={f.url}
						oninput={() => f.schedule()}
						onblur={() => (f.touched.url = true)}
						placeholder="https://claude.ai/project/…"
						aria-invalid={show('url') ? 'true' : undefined}
						aria-describedby={show('url') ? 'f-url-err' : 'f-url-hint'}
					/>
					{#if show('url')}
						<p class="err" id="f-url-err"><Icon name="cross" size="glyph" />{errors.url}</p>
					{:else}
						<p class="hint" id="f-url-hint">
							Copy and open uses it to start new conversations in the right Project.
						</p>
					{/if}
				</div>
			</div>

			<div class="form-sec">
				<h2>Variables</h2>
				{#each f.specs as v, i (v.name)}
					{@const k = `var-${v.name}`}
					<div class={['field', i >= 2 && 'half']}>
						<label for="v-{v.name}">{v.label} <code>[{v.name}]</code></label>
						{#if v.multiline}
							<textarea
								class="textarea"
								id="v-{v.name}"
								rows="2"
								bind:value={f.variables[v.name]}
								oninput={() => f.schedule()}
								onblur={() => (f.touched[k] = true)}
								placeholder={v.placeholder}
								aria-invalid={show(k) ? 'true' : undefined}
								aria-describedby={show(k) ? `v-${v.name}-err` : undefined}></textarea>
						{:else}
							<input
								class="input"
								id="v-{v.name}"
								bind:value={f.variables[v.name]}
								oninput={() => f.schedule()}
								onblur={() => (f.touched[k] = true)}
								placeholder={v.placeholder}
								aria-invalid={show(k) ? 'true' : undefined}
								aria-describedby={show(k)
									? `v-${v.name}-err`
									: v.fromSettings
										? `v-${v.name}-hint`
										: undefined}
							/>
						{/if}
						{#if show(k)}
							<p class="err" id="v-{v.name}-err">
								<Icon name="cross" size="glyph" /><span>{errors[k]}</span>
							</p>
						{:else if v.fromSettings && f.defaults[v.name]}
							<p class="hint" id="v-{v.name}-hint">From your defaults in Settings</p>
						{/if}
					</div>
				{/each}
			</div>

			<div class="form-sec" role="group" aria-labelledby="cond-h">
				<h2 id="cond-h">Conditions</h2>
				<p class="hint">
					Each one decides which steps apply. You can change them until the Stage 2 packet is
					signed; undecided blocks that sign.
				</p>
				{#each f.conditionSpecs as c (c.id)}
					<div class="cond">
						<span id="c-{c.id}"><b>{c.label}</b> <code>{c.id}</code></span>
						<p>{c.effect}</p>
						<Segmented
							options={TRI}
							value={f.conditions[c.id] ?? 'undecided'}
							labelledby="c-{c.id}"
							onchange={(v: TriState) => {
								f.conditions[c.id] = v;
								f.schedule(0);
							}}
						/>
					</div>
				{/each}
			</div>
		</div>

		<aside class="review" aria-labelledby="review-h">
			<h2 id="review-h">What Foreman will write</h2>
			{#if f.preview}
				<dl class="kv">
					<dt>Blueprint</dt>
					<dd>
						{f.preview.pin.version} at {f.preview.pin.commit}, {f.preview.pin.clean
							? 'clean working tree'
							: 'uncommitted changes'}
					</dd>
					<dt>Path</dt>
					<dd>{f.preview.path.summary}</dd>
				</dl>
				{#each [{ label: 'Create', list: groups.create, icon: 'plus' as const, tone: 'pass' as const }, { label: 'Leave alone', list: groups.leave, icon: 'check' as const, tone: 'muted' as const }, { label: 'Refuse to overwrite', list: groups.refuse, icon: 'cross' as const, tone: 'fail' as const }] as g (g.label)}
					{#if g.list.length}
						<p class="group-label">{g.label}</p>
						<dl class="writes-list" aria-label={g.label}>
							{#each g.list as w (w.path)}
								<div>
									<dt>
										<Icon name={g.icon} size="glyph" tone={g.tone} /><code
											><Text text={w.path} layout={false} /></code
										>
									</dt>
									{#if w.reason}<dd class={['why', w.action === 'create' && 'inline']}>
											{w.reason}
										</dd>{/if}
								</div>
							{/each}
						</dl>
					{/if}
				{/each}
				{#if f.preview.gitInit}<p class="hint">
						Foreman runs git init in the folder (it isn't a repository yet).
					</p>{/if}
			{:else}
				<p class="hint">
					Choose the folder to see the exact files Foreman creates, leaves alone and refuses to
					overwrite.
				</p>
			{/if}
			{#if f.previewError}<p class="err" role="alert">
					<Icon name="cross" size="glyph" />{f.previewError}
				</p>{/if}
			<div class="actions">
				<button
					class="btn primary lg"
					type="submit"
					aria-disabled={!f.valid || f.busy ? 'true' : undefined}
					aria-describedby={createWhy ? 'create-why' : undefined}>Create project</button
				>
			</div>
			{#if createWhy}<p class="reason" id="create-why">{createWhy}</p>{/if}
			{#if f.createError}<p class="err" role="alert">
					<Icon name="cross" size="glyph" />{f.createError}
				</p>{/if}
		</aside>
	</form>
</main>

<style>
	.setup {
		display: grid;
		grid-template-columns: minmax(0, 620px) minmax(280px, 380px);
		gap: var(--space-7);
		padding: var(--space-6) var(--space-7) var(--space-8);
	}
	.form-sec {
		margin-top: var(--space-6);
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-4);
	}
	.form-sec > :global(*) {
		grid-column: 1 / -1;
	}
	.form-sec > .half {
		grid-column: auto;
	}
	.form-sec > h2 {
		font-size: var(--text-15);
		font-weight: var(--weight-semibold);
		padding-bottom: var(--space-3);
		border-bottom: var(--hairline) solid var(--rule);
	}
	.cond {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: var(--space-1) var(--space-5);
		align-items: center;
		padding: 6px 0;
		border-bottom: var(--hairline) solid var(--rule);
	}
	.cond b {
		font-weight: var(--weight-medium);
	}
	.cond code {
		font-size: var(--text-12);
		color: var(--ink-muted);
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
	.review {
		position: sticky;
		top: var(--space-6);
		align-self: start;
		border: var(--hairline) solid var(--rule);
		background: var(--ground);
		padding: var(--space-5);
		display: grid;
		gap: var(--space-4);
	}
	.review h2 {
		font-size: var(--text-15);
		font-weight: var(--weight-semibold);
	}
	.writes-list {
		display: grid;
		gap: var(--space-1);
	}
	.writes-list > div {
		font-size: var(--text-12);
		padding: 3px 0;
	}
	.writes-list dt {
		display: grid;
		grid-template-columns: 16px minmax(0, 1fr);
		gap: var(--space-3);
		align-items: center;
	}
	.writes-list code {
		font-size: var(--text-12);
	}
	.why {
		display: block;
		color: var(--ink-muted);
		padding-left: 28px;
	}
	.writes-list > div:has(.inline) {
		display: flex;
		gap: var(--space-2);
		align-items: baseline;
	}
	.why.inline {
		padding-left: 0;
		color: var(--ink);
	}
	.group-label {
		font-size: var(--text-12);
		font-weight: var(--weight-semibold);
		color: var(--ink-muted);
		margin-top: var(--space-2);
	}
</style>
