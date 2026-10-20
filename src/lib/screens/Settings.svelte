<script lang="ts">
	// Settings (F-029, mockup settings.html): name for signatures, theme, editor, tool paths (picked
	// in Rust's dialog; refusals shown inline), variable defaults, terminal, environment pass-through
	// and the login PATH, and run history. Security-relevant changes are confirmed by Rust in a
	// native dialog (SR-24a); a file changed on disk since it was read is reloaded (AC-F029-03).
	import { patch } from '#lib/actions/settings.ts';
	import { failureText } from '#lib/actions/words.ts';
	import Icon from '#lib/components/Icon.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { SettingsPatch, SettingsView, Theme, ToolKind } from '#lib/ipc/types.ts';
	import { refresh } from '#lib/stores/bus.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	ui.setChrome({ title: 'Settings' });

	const settings = resource(
		() => null,
		(c) => c.settingsGet({}),
		() => ['settings']
	);
	const s: SettingsView | undefined = $derived(settings.value);

	let dri = $state('');
	let editor = $state('');
	let fontSize = $state('12');
	let scrollback = $state('10000');
	let addEnv = $state('');
	const vars = $state<Record<string, string>>({});
	let seeded = false;
	$effect(() => {
		if (!s || seeded) return;
		seeded = true;
		dri = s.driName ?? '';
		editor = s.editorBundleId ?? '';
		fontSize = String(s.terminal.fontSize);
		scrollback = String(s.terminal.scrollback);
		for (const [k, v] of Object.entries(s.globalVariables)) vars[k] = v;
	});

	const ALWAYS_REMOVED = [
		'ANTHROPIC_API_KEY',
		'ANTHROPIC_AUTH_TOKEN',
		'CLAUDE_CODE_OAUTH_TOKEN',
		'AWS_BEARER_TOKEN_BEDROCK'
	];
	const envError = $derived(
		!addEnv
			? null
			: ALWAYS_REMOVED.includes(addEnv.trim())
				? `${addEnv.trim()} can't be passed through. Foreman always removes provider keys so billing stays on your plan.`
				: !/^[A-Za-z_][A-Za-z0-9_]{0,127}$/.test(addEnv.trim())
					? 'Use a variable name: letters, digits and underscores, not starting with a digit.'
					: (s?.envPassthrough.includes(addEnv.trim()) ?? false)
						? `${addEnv.trim()} is already passed through.`
						: null
	);
	const fontError = $derived(
		!/^\d+$/.test(fontSize) || +fontSize < 9 || +fontSize > 32 ? 'Use a size from 9 to 32.' : null
	);
	const sbNumber = $derived(Number(scrollback.replace(/[,\s]|lines/g, '')));
	const sbError = $derived(
		!Number.isInteger(sbNumber) || sbNumber < 1000 || sbNumber > 100000
			? 'Use 1,000 to 100,000 lines.'
			: null
	);
	const editorError = $derived(
		editor && !/^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+){1,10}$/.test(editor)
			? 'Use an app bundle ID, for example com.microsoft.VSCode.'
			: null
	);

	async function save(p: Partial<SettingsPatch>, done: string): Promise<void> {
		if (!s) return;
		const c = await ipc();
		try {
			settings.set(await c.settingsUpdate({ expectRev: s.rev, patch: patch(p) }));
			ui.toast(done);
			refresh('settings');
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			if (f.code === 'stale') {
				await settings.reload();
				ui.say('Settings changed on disk. Foreman reloaded them; make the change again.');
			} else if (f.code === 'cancelled') ui.say('Cancelled. Nothing changed.');
			else ui.say(failureText(f));
		}
	}
	async function pick(tool: ToolKind): Promise<void> {
		const c = await ipc();
		try {
			await c.settingsPickTool({ tool });
			await settings.reload();
			ui.toast(`${tool} path saved`);
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			ui.say(
				f.code === 'cancelled' ? 'Cancelled. The last valid path stays in use.' : failureText(f)
			);
		}
	}
	async function chooseBlueprint(): Promise<void> {
		const c = await ipc();
		try {
			await c.blueprintPickRepo({ mode: 'choose' });
			await settings.reload();
			ui.toast('Blueprint repo saved');
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			ui.say(
				f.code === 'cancelled' ? 'Cancelled. The blueprint repo is unchanged.' : failureText(f)
			);
		}
	}
	async function captureAgain(): Promise<void> {
		const c = await ipc();
		try {
			const pv = await c.doctorCaptureLoginPath({});
			settings.set(await c.doctorLoginPathApply({ previewId: pv.previewId }));
			ui.toast('Login shell PATH captured');
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			ui.say(f.code === 'cancelled' ? 'Cancelled. No shell was started.' : failureText(f));
		}
	}
	async function clearRuns(): Promise<void> {
		const c = await ipc();
		try {
			await c.runsClear({});
			await settings.reload();
			ui.toast('Run history cleared');
			refresh('runs');
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			ui.say(f.code === 'cancelled' ? 'Cancelled. Run history kept.' : failureText(f));
		}
	}
	const TOOLS: ToolKind[] = ['claude', 'git'];
	const THEMES: { value: Theme; label: string }[] = [
		{ value: 'system', label: 'System' },
		{ value: 'night', label: 'Night' },
		{ value: 'vellum', label: 'Vellum' }
	];
	const SECTIONS = [
		['general', 'General'],
		['paths', 'Paths'],
		['vars', 'Variable defaults'],
		['terminal', 'Terminal'],
		['env', 'Environment'],
		['data', 'Data']
	] as const;
	let sectionAt = $state<string>('general');
	const date = (iso: string) =>
		new Date(iso).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', timeZone: 'UTC' });
</script>

<main class="main" id="main" aria-labelledby="set-title">
	<div class="settings">
		<nav class="snav" aria-label="Settings sections">
			{#each SECTIONS as [id, label] (id)}
				<a
					href="#{id}"
					aria-current={sectionAt === id ? 'true' : undefined}
					onclick={() => (sectionAt = id)}>{label}</a
				>
			{/each}
		</nav>
		<div>
			<h1 class="page-title" id="set-title">Settings</h1>
			{#if settings.error}<p class="err" role="alert">
					<Icon name="cross" size="glyph" />{failureText(settings.error)}
				</p>{/if}
			{#if s}
				<section class="sgroup" id="general" aria-labelledby="g-h">
					<h2 id="g-h">General</h2>
					<form
						class="srow"
						onsubmit={(e) => {
							e.preventDefault();
							void save({ driName: dri }, 'Name saved');
						}}
					>
						<label for="s-dri">Name for signatures</label>
						<div class="input-row">
							<input class="input" id="s-dri" bind:value={dri} autocomplete="name" />
							<button class="btn" type="submit" disabled={dri.trim() === (s.driName ?? '')}
								>Save</button
							>
						</div>
					</form>
					<div class="srow">
						<span class="field-label" id="theme-l">Theme</span>
						<Segmented
							options={THEMES}
							value={s.theme}
							labelledby="theme-l"
							onchange={(t: Theme) => {
								ui.applyTheme(t);
								void save(
									{ theme: t },
									`Theme: ${t === 'system' ? 'follows the system' : t === 'night' ? 'Night' : 'Vellum'}`
								);
							}}
						/>
					</div>
					<form
						class="srow"
						onsubmit={(e) => {
							e.preventDefault();
							if (!editorError)
								void save(
									{ editor: editor ? { kind: 'set', bundleId: editor } : { kind: 'clear' } },
									editor ? 'Editor saved' : 'Editor cleared'
								);
						}}
					>
						<label for="s-editor">Editor for Open in editor</label>
						<div class="field">
							<div class="input-row">
								<input
									class="input mono"
									id="s-editor"
									bind:value={editor}
									placeholder="com.microsoft.VSCode"
									aria-invalid={editorError ? 'true' : undefined}
									aria-describedby={editorError ? 's-editor-err' : 's-editor-hint'}
								/>
								<button
									class="btn"
									type="submit"
									disabled={!!editorError || editor === (s.editorBundleId ?? '')}>Save</button
								>
							</div>
							{#if editorError}
								<p class="err" id="s-editor-err"><Icon name="cross" size="glyph" />{editorError}</p>
							{:else}
								<p class="hint" id="s-editor-hint">
									{s.editorBundleId
										? 'Foreman opens .md, .json and .txt files with this app only.'
										: 'Not set: Open in editor stays off until you set one.'}
								</p>
							{/if}
						</div>
					</form>
				</section>

				<section class="sgroup" id="paths" aria-labelledby="p-h">
					<h2 id="p-h">Paths</h2>
					<div class="srow">
						<label for="s-bp">Blueprint repo</label>
						<div class="field">
							<div class="input-row">
								<input
									class="input mono"
									id="s-bp"
									readonly
									value={s.blueprintRepo?.displayPath ?? 'Not set'}
								/>
								<button class="btn" type="button" onclick={chooseBlueprint}>Choose…</button>
							</div>
							{#if s.blueprintRepo}<p class="hint">
									Blueprint {s.blueprintRepo.version} at {s.blueprintRepo.commit}{s.blueprintRepo
										.dirty
										? ', uncommitted changes'
										: ''}
								</p>{/if}
						</div>
					</div>
					{#each TOOLS as tool (tool)}
						{@const t = s.tools[tool]}
						<div class="srow">
							<label for="s-{tool}">{tool}</label>
							<div class="field">
								<div class="input-row">
									<input
										class="input mono"
										id="s-{tool}"
										readonly
										value={t.path ?? 'Not found'}
										aria-invalid={t.problem ? 'true' : undefined}
										aria-describedby="s-{tool}-msg"
									/>
									<button class="btn" type="button" onclick={() => pick(tool)}>Choose…</button>
								</div>
								{#if t.problem}
									<p class="err" id="s-{tool}-msg">
										<Icon name="cross" size="glyph" /><span>{t.problem}</span>
									</p>
								{:else}
									<p class="hint" id="s-{tool}-msg">
										<Icon name="check" size="glyph" tone="pass" />
										{t.autoDetected ? 'Found' : 'Chosen'}, version {t.version ?? 'unknown'}
									</p>
								{/if}
							</div>
						</div>
					{/each}
				</section>

				<section class="sgroup" id="vars" aria-labelledby="v-h">
					<h2 id="v-h">Variable defaults</h2>
					<p class="hint">New projects start with these values; each project can change them.</p>
					{#each Object.keys(vars) as k (k)}
						<div class="srow">
							<label for="s-var-{k}"><code>[{k}]</code></label>
							<input
								class="input"
								id="s-var-{k}"
								bind:value={vars[k]}
								onchange={() => save({ globalVariables: { ...vars } }, 'Defaults saved')}
							/>
						</div>
					{/each}
				</section>

				<section class="sgroup" id="terminal" aria-labelledby="t-h">
					<h2 id="t-h">Terminal</h2>
					<div class="srow">
						<label for="s-font">Font size</label>
						<div class="field">
							<input
								class="input narrow"
								id="s-font"
								inputmode="numeric"
								bind:value={fontSize}
								aria-invalid={fontError ? 'true' : undefined}
								aria-describedby={fontError ? 's-font-err' : undefined}
								onchange={() =>
									!fontError &&
									!sbError &&
									save({ terminal: { ...s.terminal, fontSize: +fontSize } }, 'Font size saved')}
							/>
							{#if fontError}<p class="err" id="s-font-err">
									<Icon name="cross" size="glyph" />{fontError}
								</p>{/if}
						</div>
					</div>
					<div class="srow">
						<label for="s-sb">Scrollback per tab</label>
						<div class="field">
							<input
								class="input mid"
								id="s-sb"
								inputmode="numeric"
								bind:value={scrollback}
								aria-invalid={sbError ? 'true' : undefined}
								aria-describedby={sbError ? 's-sb-err' : 's-sb-hint'}
								onchange={() =>
									!sbError &&
									!fontError &&
									save({ terminal: { ...s.terminal, scrollback: sbNumber } }, 'Scrollback saved')}
							/>
							{#if sbError}
								<p class="err" id="s-sb-err"><Icon name="cross" size="glyph" />{sbError}</p>
							{:else}
								<p class="hint" id="s-sb-hint">
									Older lines are archived with the run; the last 2,000 are kept for 90 days.
								</p>
							{/if}
						</div>
					</div>
					<div class="srow">
						<span class="field-label" id="s-sr-l">Screen reader mode</span>
						<div class="toggle">
							<button
								type="button"
								class="switch"
								role="switch"
								aria-checked={s.screenReaderMode}
								aria-labelledby="s-sr-l"
								aria-describedby="s-sr-hint"
								onclick={() =>
									save(
										{ screenReaderMode: !s.screenReaderMode },
										s.screenReaderMode ? 'Screen reader mode off' : 'Screen reader mode on'
									)}
							></button>
							<span class="hint" id="s-sr-hint"
								>CLI launches add --ax-screen-reader and the terminal reads output aloud.</span
							>
						</div>
					</div>
				</section>

				<section class="sgroup" id="env" aria-labelledby="e-h">
					<h2 id="e-h">Environment</h2>
					<div class="srow">
						<span class="field-label" id="s-env-l">Pass through to Claude</span>
						<div class="field">
							<ul class="taglist" aria-labelledby="s-env-l">
								{#each s.envPassthrough as name (name)}
									<li class="tag">
										{name}<button
											type="button"
											aria-label="Remove {name}"
											onclick={() =>
												save(
													{ envPassthrough: s.envPassthrough.filter((x) => x !== name) },
													`${name} removed`
												)}>×</button
										>
									</li>
								{/each}
							</ul>
							<form
								class="input-row"
								onsubmit={(e) => {
									e.preventDefault();
									if (!envError && addEnv.trim())
										void save(
											{ envPassthrough: [...s.envPassthrough, addEnv.trim()] },
											`${addEnv.trim()} added`
										).then(() => (addEnv = ''));
								}}
							>
								<label class="vh" for="s-addenv">Add a variable</label>
								<input
									class="input mono"
									id="s-addenv"
									bind:value={addEnv}
									placeholder="VARIABLE_NAME"
									aria-invalid={envError ? 'true' : undefined}
									aria-describedby={envError ? 's-env-err' : undefined}
								/>
								<button class="btn" type="submit" disabled={!!envError || !addEnv.trim()}
									>Add</button
								>
							</form>
							{#if envError}<p class="err" id="s-env-err">
									<Icon name="cross" size="glyph" /><span>{envError}</span>
								</p>{/if}
							<p class="hint">Adding or removing a name asks you once more in a macOS dialog.</p>
						</div>
					</div>
					<div class="srow">
						<span class="field-label">Login shell PATH</span>
						<div class="field">
							<p class="hint">
								{#if s.loginPath}Captured {date(s.loginPath.capturedAt)}:
									<span class="mono">{s.loginPath.entries.join(':')}</span>{:else}Not captured.
									Foreman uses a fixed PATH.{/if}
							</p>
							<div class="actions">
								<button class="btn" type="button" onclick={captureAgain}
									>{s.loginPath ? 'Capture again' : 'Capture my login shell PATH'}</button
								>
								{#if s.loginPath}<button
										class="btn ghost"
										type="button"
										onclick={() => save({ clearLoginPath: true }, 'Login PATH cleared')}
										>Clear</button
									>{/if}
							</div>
						</div>
					</div>
				</section>

				<section class="sgroup" id="data" aria-labelledby="d-h">
					<h2 id="d-h">Data</h2>
					<div class="srow">
						<span class="field-label">Run history</span>
						<div class="field">
							<p class="hint">
								{s.runHistory.runs} runs, {s.runHistory.megabytes} MB in app data. Clearing keeps stage
								records in your repos.
							</p>
							<div class="actions">
								<button class="btn danger" type="button" onclick={clearRuns}
									>Clear run history…</button
								>
							</div>
						</div>
					</div>
				</section>
			{/if}
		</div>
	</div>
</main>

<style>
	.settings {
		display: grid;
		grid-template-columns: 180px minmax(0, 680px);
		gap: var(--space-7);
		padding: var(--space-6) var(--space-7) var(--space-8);
	}
	.snav {
		display: grid;
		gap: var(--space-1);
		align-content: start;
		position: sticky;
		top: var(--space-6);
	}
	.snav a {
		padding: 5px var(--space-3);
		border-radius: var(--radius-control);
		color: var(--ink);
		font-size: var(--text-13);
		text-decoration: none;
	}
	.snav a[aria-current] {
		background: var(--selected);
		color: var(--accent-ink);
		font-weight: var(--weight-medium);
	}
	.sgroup {
		margin-top: var(--space-6);
		display: grid;
		gap: var(--space-4);
	}
	.sgroup:first-of-type {
		margin-top: var(--space-4);
	}
	.sgroup > h2 {
		font-size: var(--text-15);
		font-weight: var(--weight-semibold);
		padding-bottom: var(--space-3);
		border-bottom: var(--hairline) solid var(--rule);
	}
	.srow {
		display: grid;
		grid-template-columns: 200px minmax(0, 1fr);
		gap: var(--space-4);
		align-items: start;
	}
	.srow > label,
	.srow > .field-label {
		padding-top: 5px;
		font-size: var(--text-13);
		font-weight: var(--weight-medium);
	}
	.taglist {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}
	.tag {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--text-12);
		padding: 1px var(--space-2) 1px 7px;
		border: 1px solid var(--control-edge);
		border-radius: var(--radius-pill);
	}
	.tag button {
		border: 0;
		background: transparent;
		color: var(--ink-muted);
		padding: 0 var(--space-1);
	}
	.narrow {
		width: 80px;
	}
	.mid {
		width: 160px;
	}
	.mono {
		overflow-wrap: anywhere;
	}
</style>
