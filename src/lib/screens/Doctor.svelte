<script lang="ts">
	// Doctor (F-019, mockup doctor.html): each check with its status, what was found and the fix in
	// plain words (AC-F019-04, -06); the capability table and contract probes (P2); toolchain and the
	// consented login-shell PATH capture (P2, SR-02: Rust asks in a native dialog first).
	import { failureText } from '#lib/actions/words.ts';
	import Icon from '#lib/components/Icon.svelte';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { DoctorCheck, LoginPathPreview } from '#lib/ipc/types.ts';
	import { refresh } from '#lib/stores/bus.svelte.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';
	import Text from '#lib/render/Text.svelte';

	const report = resource(
		() => null,
		(c) => c.doctorRun({ scope: 'full' }),
		() => ['doctor']
	);
	const r = $derived(report.value);
	const time = $derived(r ? new Date(r.ranAt).toISOString().slice(11, 16) : '');
	$effect(() => {
		ui.setChrome({ title: 'Doctor', sub: r ? `Last run ${time}, at launch` : '' });
	});
	const problems = $derived(r?.checks.filter((c) => c.level === 'failing').length ?? 0);
	let preview = $state<LoginPathPreview | null>(null);

	const look = (c: DoctorCheck) =>
		c.level === 'ok'
			? ({ icon: 'check', tone: 'pass' } as const)
			: c.level === 'failing'
				? ({ icon: 'cross', tone: 'fail' } as const)
				: c.level === 'warning'
					? ({ icon: 'warn', tone: 'attention' } as const)
					: ({ icon: 'info', tone: 'muted' } as const);

	async function capture(): Promise<void> {
		const c = await ipc();
		try {
			preview = await c.doctorCaptureLoginPath({});
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			ui.say(f.code === 'cancelled' ? 'Cancelled. No shell was started.' : failureText(f));
		}
	}
	async function apply(): Promise<void> {
		if (!preview) return;
		const c = await ipc();
		try {
			await c.doctorLoginPathApply({ previewId: preview.previewId });
			preview = null;
			ui.toast('Login shell PATH saved');
			refresh('doctor', 'settings');
			void report.reload();
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			ui.say(f.code === 'cancelled' ? 'Cancelled. PATH unchanged.' : failureText(f));
		}
	}
</script>

{#snippet checks(list: DoctorCheck[])}
	<ul class="checks">
		{#each list as c (c.id)}
			{@const l = look(c)}
			<li class={[c.level === 'failing' && 'is-fail']}>
				<Icon name={l.icon} size="glyph" tone={l.tone} />
				<span class="name">{c.name}<span class="vh">: {c.level === 'ok' ? 'passes' : c.level}</span></span>
				<span class="detail"><Text spans={c.detail} /></span>
				{#if c.action === 'captureLoginPath'}
					<button class="btn" type="button" onclick={capture}>Capture my login shell PATH</button>
				{:else}
					<span class={['found', c.foundMono && 'mono']}>{c.found ?? '—'}</span>
				{/if}
			</li>
		{/each}
	</ul>
{/snippet}

<main class="main" id="main" aria-labelledby="doc-title">
	<div class="page wide">
		<div class="head">
			<div>
				<h1 class="page-title" id="doc-title">Doctor</h1>
				<p class="page-sub">
					What Foreman needs on this Mac to launch steps exactly as the blueprint says.
					{#if problems}{problems} {problems === 1 ? 'problem blocks' : 'problems block'} some steps.{:else if r}Everything
						it checks passes.{/if}
				</p>
			</div>
			<button class="btn" type="button" onclick={() => report.reload()}><Icon name="restore" />Check again</button>
		</div>
		{#if report.error}
			<p class="err" role="alert"><Icon name="cross" size="glyph" />{failureText(report.error)}</p>
		{/if}
		{#if r}
			<section class="section" aria-labelledby="env-h">
				<div class="section-head"><h2 class="section-title" id="env-h">Claude Code</h2></div>
				{@render checks(r.checks.filter((c) => c.group === 'claude'))}
			</section>

			<div class="cols">
				<section class="section" aria-labelledby="cap-h">
					<div class="section-head">
						<h2 class="section-title" id="cap-h">Capability table</h2>
						<span class="muted">What your version supports</span>
					</div>
					<table class="table cap">
						<thead>
							<tr><th scope="col">Feature</th><th scope="col">Minimum</th><th scope="col">Found</th><th scope="col">Blocks</th></tr>
						</thead>
						<tbody>
							{#each r.capabilities as cap (cap.feature)}
								<tr class={[!cap.found && 'blocks']}>
									<td>{#if cap.code}<code>{cap.feature}</code>{:else}{cap.feature}{/if}</td>
									<td>{cap.minimum}</td>
									<td>
										{#if cap.found}<span class="status pass"><Icon name="check" size="glyph" />yes</span>
										{:else}<span class="status fail"><Icon name="cross" size="glyph" />no</span>{/if}
									</td>
									<td class={[!cap.blocks && 'muted']}>{cap.blocks ?? '—'}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</section>
				<section class="section" aria-labelledby="probe-h">
					<div class="section-head"><h2 class="section-title" id="probe-h">Contract probes</h2></div>
					<p class="page-sub small">Run in the trusted probe folder when the version changes. They cost no tokens.</p>
					<table class="table probes">
						<thead><tr><th scope="col">Probe</th><th scope="col">Result</th></tr></thead>
						<tbody>
							{#each r.probes as p, i (i)}
								<tr>
									<td><Text spans={p.probe} /></td>
									<td>
										<span class={['status', p.result === 'pass' ? 'pass' : p.result === 'fail' ? 'fail' : 'attention']}
											><Icon name={p.result === 'pass' ? 'check' : p.result === 'fail' ? 'cross' : 'clock'} size="glyph" />{p.text}</span
										>
										{#if p.detail}<div class="muted">{p.detail}</div>{/if}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
					{#if r.probeNote}<p class="remap note">{r.probeNote}</p>{/if}
				</section>
			</div>

			{#if r.checks.some((c) => c.group === 'toolchain')}
				<section class="section" aria-labelledby="tool-h">
					<div class="section-head"><h2 class="section-title" id="tool-h">Toolchain and PATH</h2></div>
					{@render checks(r.checks.filter((c) => c.group === 'toolchain'))}
				</section>
			{/if}
		{:else if report.loading}
			<p class="page-sub" role="status">Checking this Mac…</p>
		{/if}
	</div>
</main>

{#if preview}
	<div class="scrim" aria-hidden="true"></div>
	<div class="dialog" role="dialog" aria-modal="true" aria-labelledby="lp-title" aria-describedby="lp-desc">
		<div class="dialog-head">
			<h2 class="dialog-title" id="lp-title">Login shell PATH from {preview.shell}</h2>
			<p class="hint" id="lp-desc">Foreman keeps only absolute entries. Save asks you once more in a macOS dialog.</p>
		</div>
		<div class="dialog-body">
			<ul class="writes">
				{#each preview.entries as e, i (i)}
					<li>
						<span class="what"
							>{#if e.ok}<Icon name="check" size="glyph" tone="pass" /> Keep{:else}<Icon name="cross" size="glyph" tone="fail" /> Drop{/if}</span
						>
						<span><code><Text text={e.entry} /></code>{#if e.why}<span class="muted"> {e.why}</span>{/if}</span>
					</li>
				{/each}
			</ul>
		</div>
		<div class="dialog-foot">
			<span class="spacer"></span>
			<button class="btn" type="button" onclick={() => (preview = null)}>Cancel</button>
			<!-- svelte-ignore a11y_autofocus -->
			<button class="btn primary" type="button" onclick={apply} autofocus>Save PATH</button>
		</div>
	</div>
{/if}

<style>
	.wide {
		max-width: 1120px;
	}
	.head {
		display: flex;
		align-items: end;
		gap: var(--space-4);
	}
	.head > .btn {
		margin-left: auto;
	}
	.checks li {
		display: grid;
		grid-template-columns: 22px 220px minmax(0, 1fr) auto;
		gap: var(--space-4);
		align-items: start;
		padding: 10px var(--space-2);
		border-bottom: var(--hairline) solid var(--rule);
	}
	.checks li.is-fail {
		background: var(--fail-wash);
		box-shadow: inset 3px 0 0 var(--fail);
	}
	.checks :global(.icon.glyph) {
		width: 14px;
		height: 14px;
		margin-top: var(--space-1);
	}
	.name {
		font-weight: var(--weight-medium);
	}
	.detail {
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.detail :global(strong) {
		color: var(--ink);
		font-weight: var(--weight-medium);
	}
	.found {
		font-size: var(--text-12);
	}
	.cap td:nth-child(2),
	.cap td:nth-child(3) {
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.cap tr.blocks td {
		background: var(--fail-wash);
	}
	.cols {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		gap: var(--space-7);
	}
	.small {
		font-size: var(--text-12);
	}
	.probes {
		margin-top: var(--space-3);
	}
	.note {
		margin-top: var(--space-3);
	}
</style>
