<script lang="ts">
	// First run (F-019, mockup first-run.html): blueprint repo → template check → your name → Doctor
	// (with the probe-folder trust step) → start your first project (AC-F019-01…03). Auto-detection
	// checks only Rust's fixed candidates; Choose… is a Rust dialog (SR-31a, SR-24).
	import { onMount } from 'svelte';

	import { patch } from '#lib/actions/settings.ts';
	import { failureText } from '#lib/actions/words.ts';
	import Icon from '#lib/components/Icon.svelte';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { BlueprintRepoInfo, DoctorReport, TemplateHealth } from '#lib/ipc/types.ts';
	import { to } from '#lib/nav/routes.ts';
	import { pendingOutput, terminal } from '#lib/stores/terminal.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';
	import Text from '#lib/render/Text.svelte';

	ui.setChrome({ title: 'Welcome to Foreman' });

	let repo = $state<BlueprintRepoInfo | null>(null);
	let repoError = $state<string | null>(null);
	let health = $state<TemplateHealth | null>(null);
	let name = $state('');
	let savedName = $state<string | null>(null);
	let editingName = $state(false);
	let doctor = $state<DoctorReport | null>(null);
	let trust = $state<'todo' | 'opened' | 'skipped'>('todo');
	let busy = $state(false);

	async function detect(mode: 'autoDetect' | 'choose'): Promise<void> {
		const c = await ipc();
		repoError = null;
		try {
			repo = await c.blueprintPickRepo({ mode });
			health = await c.templateHealth({ projectId: null });
			const s = await c.settingsGet({});
			savedName = s.driName;
			name = s.driName ?? '';
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			repoError =
				f.code === 'notFound'
					? 'No blueprint repo at ~/claude-blueprint or ~/Developer/claude-blueprint. Choose its folder.'
					: f.code === 'cancelled'
						? 'No folder chosen.'
						: failureText(f);
		}
	}

	async function saveName(): Promise<void> {
		const c = await ipc();
		const s = await c.settingsGet({});
		try {
			const v = await c.settingsUpdate({
				expectRev: s.rev,
				patch: patch({ driName: name.trim() })
			});
			savedName = v.driName;
			editingName = false;
			await runDoctor();
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}

	async function runDoctor(): Promise<void> {
		const c = await ipc();
		busy = true;
		doctor = await c.doctorRun({ scope: 'p1' });
		busy = false;
	}

	async function openProbe(): Promise<void> {
		const c = await ipc();
		const out = pendingOutput();
		try {
			const r = await c.probeFolderOpen({}, out.cb);
			out.bind(r.runId);
			terminal.add(r, null);
			trust = 'opened';
			ui.toast('Probe folder opened in a terminal tab. Answer Claude Code there.');
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}

	onMount(() => {
		void detect('autoDetect').then(() => (savedName ? runDoctor() : undefined));
	});

	const step = $derived(
		!repo ? 1 : !health ? 2 : !savedName || editingName ? 3 : !doctor || trust === 'todo' ? 4 : 5
	);
	const checks = $derived((doctor?.checks ?? []).filter((c) => c.id !== 'path'));
	const doctorOk = $derived(!!doctor && trust !== 'todo');
	const tone = (l: string) =>
		l === 'ok' ? 'pass' : l === 'failing' ? 'fail' : l === 'warning' ? 'attention' : 'muted';
</script>

<main class="main" id="main" aria-labelledby="fr-title">
	<div class="fr">
		<div class="fr-hero">
			<h1 id="fr-title">Set up the site office</h1>
			<p>
				Foreman reads your blueprint, checks this Mac, and then you start your first project. It
				never edits the blueprint or your Claude settings.
			</p>
		</div>
		<ol class="fr-steps">
			<li class={step > 1 ? 'done' : 'now'} aria-current={step === 1 ? 'step' : undefined}>
				<span class="fr-n" aria-hidden="true"
					>{#if step > 1}<Icon name="check" size="glyph" tone="pass" />{:else}1{/if}</span
				>
				<div>
					<div class="fr-t">{repo ? 'Blueprint repo' : 'Choose your blueprint repo'}</div>
					{#if repo}
						<div class="fr-d">
							Found {repo.displayPath}. Blueprint {repo.version} at {repo.commit}, grammar v{repo.grammarVersion}.
						</div>
					{:else}
						<div class="fr-d">
							Foreman checks ~/claude-blueprint and ~/Developer/claude-blueprint only.
						</div>
						{#if repoError}<p class="err" role="alert">
								<Icon name="cross" size="glyph" />{repoError}
							</p>{/if}
						<div class="fr-body actions">
							<button class="btn primary" type="button" onclick={() => detect('choose')}
								>Choose…</button
							>
						</div>
					{/if}
				</div>
			</li>
			<li
				class={step > 2 ? 'done' : step === 2 ? 'now' : 'later'}
				aria-current={step === 2 ? 'step' : undefined}
			>
				<span class="fr-n" aria-hidden="true"
					>{#if step > 2}<Icon name="check" size="glyph" tone="pass" />{:else}2{/if}</span
				>
				<div>
					<div class="fr-t">Template health</div>
					{#if health}
						<div class="fr-d">
							Stages {repo?.health.runnable ?? '0 to 5'} runnable. Stages {repo?.health
								.needsAnnotation ?? '6 to 18'} need annotation.
							{health.counts.errors} errors, {health.counts.warnings} warnings.
						</div>
						{#if health.diagnostics.length}
							<details class="diags">
								<summary
									><Icon name="chev" class="chev" />Show the {health.diagnostics.length} diagnostics</summary
								>
								<ul>
									{#each health.diagnostics as d, i (i)}
										<li>
											<Icon
												name={d.severity === 'error' ? 'cross' : 'warn'}
												size="glyph"
												tone={d.severity === 'error' ? 'fail' : 'attention'}
											/>
											<span class="mono loc">{d.line}:{d.column}</span>
											<span><Text spans={d.message} /></span>
										</li>
									{/each}
								</ul>
							</details>
						{/if}
					{:else}
						<div class="fr-d">Checks every fm: annotation in the blueprint.</div>
					{/if}
				</div>
			</li>
			<li
				class={step > 3 ? 'done' : step === 3 ? 'now' : 'later'}
				aria-current={step === 3 ? 'step' : undefined}
			>
				<span class="fr-n" aria-hidden="true"
					>{#if step > 3}<Icon name="check" size="glyph" tone="pass" />{:else}3{/if}</span
				>
				<div>
					<div class="fr-t">Your name for signatures</div>
					{#if step > 3}
						<div class="fr-d">
							{savedName}, from git config user.name
							<button class="btn ghost xs" type="button" onclick={() => (editingName = true)}
								>Change</button
							>
						</div>
					{:else if step === 3}
						<form
							class="fr-body name"
							onsubmit={(e) => {
								e.preventDefault();
								void saveName();
							}}
						>
							<div class="field">
								<label for="fr-name">Name</label>
								<input class="input" id="fr-name" bind:value={name} autocomplete="name" />
								<p class="hint">
									Prefilled from git config user.name. Foreman writes it only when you sign.
								</p>
							</div>
							<div class="actions">
								<button class="btn primary" type="submit" disabled={!name.trim()}>Save name</button>
							</div>
						</form>
					{/if}
				</div>
			</li>
			<li
				class={step > 4 ? 'done' : step === 4 ? 'now' : 'later'}
				aria-current={step === 4 ? 'step' : undefined}
			>
				<span class="fr-n" aria-hidden="true"
					>{#if step > 4}<Icon name="check" size="glyph" tone="pass" />{:else}4{/if}</span
				>
				<div>
					<div class="fr-t">Doctor</div>
					<div class="fr-d">Checks Claude Code, your sign-in, and git.</div>
					{#if doctor && step >= 4}
						<div class="fr-body">
							<ul class="writes checks" aria-label="Doctor checks">
								{#each checks as ch (ch.id)}
									<li>
										<span class="what"
											><Icon
												name={ch.level === 'ok'
													? 'check'
													: ch.level === 'failing'
														? 'cross'
														: 'warn'}
												size="glyph"
												tone={tone(ch.level)}
											/>
											{ch.id === 'version'
												? 'Claude Code'
												: ch.id === 'auth'
													? 'Sign-in'
													: ch.id === 'path'
														? 'Path'
														: ch.name}</span
										>
										<span>
											{#if ch.id === 'version'}{ch.found} at /opt/homebrew/bin/claude{:else if ch.id === 'auth'}claude.ai
												(your plan){:else}{ch.found}{/if}
											{#if ch.level === 'failing'}<span class="err"><Text spans={ch.detail} /></span
												>{/if}
										</span>
									</li>
								{/each}
							</ul>
							{#if trust === 'todo'}
								<div class="trust" role="group" aria-labelledby="trust-h">
									<strong id="trust-h">Trust the probe folder once</strong>
									<span>
										Foreman checks that Claude Code still accepts its flags by running it in its own
										empty folder. Claude Code asks whether to trust that folder the first time.
										Foreman opens it in a terminal tab so you can answer.
									</span>
									<code>~/Library/Application Support/dev.foreman/probe</code>
									<div class="actions">
										<button class="btn primary" type="button" onclick={openProbe}
											><Icon name="terminal" />Open in terminal and trust</button
										>
										<button class="btn ghost" type="button" onclick={() => (trust = 'skipped')}
											>Skip for now</button
										>
									</div>
									<span class="muted">Skipping keeps contract probes off; Doctor reminds you.</span>
								</div>
							{:else}
								<p class="fr-d">
									{trust === 'opened'
										? 'Probe folder opened in the terminal drawer.'
										: 'Probe folder skipped; contract probes stay off.'}
								</p>
							{/if}
						</div>
					{:else if busy}
						<p class="fr-d" role="status">Checking…</p>
					{/if}
				</div>
			</li>
			<li class={step === 5 ? 'now' : 'later'} aria-current={step === 5 ? 'step' : undefined}>
				<span class="fr-n" aria-hidden="true">5</span>
				<div>
					<div class="fr-t">Start your first project</div>
					<div class="empty-start">
						<span class="fr-d">
							No projects yet. A project is one product in its own folder; Foreman tailors the
							blueprint to it.
						</span>
						<div class="actions">
							{#if doctorOk}
								<a class="btn primary" href={to.setup()}>Start your first project<kbd>⌘N</kbd></a>
							{:else}
								<button class="btn" type="button" aria-disabled="true" aria-describedby="start-why"
									>Start your first project<kbd>⌘N</kbd></button
								>
								<span class="reason" id="start-why">Available after Doctor</span>
							{/if}
						</div>
					</div>
				</div>
			</li>
		</ol>
	</div>
</main>

<style>
	.fr {
		display: grid;
		grid-template-columns: minmax(0, 720px);
		padding: var(--space-8) var(--space-8) var(--space-8) 96px;
	}
	.fr-hero {
		display: grid;
		gap: var(--space-3);
		margin-bottom: var(--space-7);
	}
	.fr-hero h1 {
		font-size: var(--text-28);
		font-weight: var(--weight-semibold);
		letter-spacing: -0.015em;
		line-height: 1.15;
	}
	.fr-hero p {
		color: var(--ink-muted);
		max-width: 60ch;
	}
	.fr-steps > li {
		display: grid;
		grid-template-columns: 40px minmax(0, 1fr);
		gap: var(--space-4);
		padding: var(--space-4) 0;
		border-top: var(--hairline) solid var(--rule);
	}
	.fr-steps > li:last-child {
		border-bottom: var(--hairline) solid var(--rule);
	}
	.fr-n {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		border: 1px solid var(--control-edge);
		display: grid;
		place-items: center;
		font-size: var(--text-12);
		font-weight: var(--weight-semibold);
	}
	li.done .fr-n {
		border-color: var(--pass);
		color: var(--pass);
	}
	li.now .fr-n {
		border-color: var(--accent-ink);
		color: var(--accent-ink);
		border-width: 1.5px;
	}
	li.later {
		color: var(--ink-muted);
	}
	.fr-t {
		font-weight: var(--weight-semibold);
		font-size: var(--text-15);
	}
	.fr-d {
		font-size: var(--text-12);
		color: var(--ink-muted);
		margin-top: var(--space-1);
	}
	.fr-body {
		margin-top: var(--space-4);
		display: grid;
		gap: var(--space-4);
	}
	.name {
		max-width: 360px;
	}
	.trust {
		border: var(--hairline) solid var(--rule);
		background: var(--ground);
		padding: var(--space-4);
		display: grid;
		gap: var(--space-3);
		font-size: var(--text-12);
	}
	.trust strong {
		font-size: var(--text-13);
	}
	.checks :global(li) {
		grid-template-columns: 120px minmax(0, 1fr);
	}
	.empty-start {
		display: grid;
		gap: var(--space-3);
		margin-top: var(--space-3);
	}
	.diags {
		margin-top: var(--space-2);
		font-size: var(--text-12);
	}
	.diags summary {
		list-style: none;
		display: inline-flex;
		gap: 6px;
		align-items: center;
		color: var(--ink-muted);
	}
	.diags summary::-webkit-details-marker {
		display: none;
	}
	.diags ul {
		display: grid;
		gap: var(--space-2);
		margin-top: var(--space-2);
	}
	.diags li {
		display: grid;
		grid-template-columns: 16px 64px minmax(0, 1fr);
		gap: var(--space-3);
	}
	.loc {
		color: var(--ink-muted);
	}
</style>
