<script lang="ts">
	// Packet view (F-013, mockup packet.html): header line, the fields as a readable list with each
	// problem inline and its fixes (Repair P2, Open in editor, Copy fix request), a summary that the
	// watcher re-validates (role="status"), and the inspector's raw file and Stage 2's Confirm
	// conditions box. Packet text is rendered from typed nodes only (SR-26).
	import { report, runPrepared } from '#lib/actions/run.ts';
	import { failureText } from '#lib/actions/words.ts';
	import Icon from '#lib/components/Icon.svelte';
	import Inspector from '#lib/components/Inspector.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { ConditionId, PacketView, ProjectView, TriState } from '#lib/ipc/types.ts';
	import Diff from '#lib/render/Diff.svelte';
	import Doc from '#lib/render/Doc.svelte';
	import Text from '#lib/render/Text.svelte';
	import { refresh } from '#lib/stores/bus.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	let { project, stageNo }: { project: ProjectView; stageNo: number } = $props();
	const pid = $derived(project.summary.id);
	const stage = $derived(project.stages.find((s) => s.stage === stageNo) ?? null);
	const packet = resource(
		() => ({ projectId: pid, stage: stageNo }),
		(c, a) => c.packetView(a),
		(a) => [`packet:${a.projectId}:${a.stage}`, `project:${a.projectId}`]
	);
	const pk: PacketView | undefined = $derived(packet.value);
	$effect(() => {
		ui.setChrome({
			title: project.summary.name,
			sub: pk?.path ?? `docs/handoff/stage-${String(stageNo).padStart(2, '0')}.md`,
			inspector: true
		});
	});

	const problems = $derived(pk?.problems.length ?? 0);
	const settings = resource(
		() => null,
		(c) => c.settingsGet({}),
		() => ['settings']
	);
	const noEditor = $derived(settings.value ? !settings.value.editorBundleId : false);
	const isChat = $derived(
		stage?.steps.some(
			(s) => s.surface === 'chat' && (s.specLine?.note?.includes('packet') ?? false)
		) ?? false
	);
	const undecided = $derived(
		stageNo === 2
			? (
					Object.entries(project.conditions) as [
						ConditionId,
						{ value: TriState; late: boolean; locked: boolean }
					][]
				).filter(([, v]) => !v.late && v.value === 'undecided')
			: []
	);

	async function openInEditor(): Promise<void> {
		if (!pk) return;
		const c = await ipc();
		try {
			await c.fileOpen({ projectId: pid, path: pk.path });
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	async function repair(): Promise<void> {
		const c = await ipc();
		try {
			const prep = await c.packetRepairPrepare({ projectId: pid, stage: stageNo });
			report(
				await runPrepared({ projectId: pid, step: '', surface: 'cli', entry: 'stageView' }, prep)
			);
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	async function fixRequest(): Promise<void> {
		const c = await ipc();
		try {
			await c.clipboardCopy({
				projectId: pid,
				target: 'fixRequest',
				step: null,
				chain: null,
				path: null,
				stage: stageNo
			});
			ui.toast('Fix request copied. Paste it into the chat that wrote the packet.', 'pass', 'copy');
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	async function act(kind: 'normalize' | 'confirmSignature' | 'restore'): Promise<void> {
		if (!pk?.sha256) return;
		const c = await ipc();
		const req = { projectId: pid, stage: stageNo, seenSha256: pk.sha256 };
		try {
			const v =
				kind === 'normalize'
					? await c.packetNormalizeApply(req)
					: kind === 'confirmSignature'
						? await c.packetSignatureConfirm(req)
						: await c.tamperRestore(req);
			packet.set(v);
			ui.toast(
				kind === 'normalize'
					? 'Packet normalized'
					: kind === 'restore'
						? 'Signed packet restored from its commit'
						: 'Signature confirmed'
			);
			refresh(`project:${pid}`);
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			if (f.code === 'stale') {
				await packet.reload();
				ui.say('The packet changed on disk. Review the fresh version.');
			} else ui.say(f.code === 'cancelled' ? 'Cancelled. Nothing changed.' : failureText(f));
		}
	}
	async function setCondition(id: ConditionId, to: TriState): Promise<void> {
		const c = await ipc();
		try {
			await c.conditionSet({
				projectId: pid,
				condition: id,
				to,
				note: 'Confirmed at the Stage 2 packet'
			});
			ui.toast(`${id} = ${to}`);
			refresh(`project:${pid}`);
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	const TRI: { value: TriState; label: string }[] = [
		{ value: 'yes', label: 'Yes' },
		{ value: 'no', label: 'No' },
		{ value: 'undecided', label: 'Undecided' }
	];
</script>

{#if stage}
	<div class="body">
		<main class="main" id="main" aria-labelledby="pk-title">
			<div class="page narrow">
				<div class="packet-head">
					<h1 class="page-title" id="pk-title">Stage {stageNo} packet</h1>
					{#if pk?.header}<span class="packet-header-line" aria-label="Packet header"
							><Text text={pk.header.line} /></span
						>{/if}
				</div>
				{#if packet.error}
					<p class="err" role="alert">
						<Icon name="cross" size="glyph" />{failureText(packet.error)}
					</p>
				{:else if pk && !pk.exists}
					<div class="empty">
						<strong>No Stage {stageNo} packet yet</strong>
						<span
							><code>{pk.path}</code> doesn't exist. The stage's last step writes it; Foreman checks it
							as soon as it lands.</span
						>
					</div>
				{:else if pk}
					{#if pk.writtenBy}<p class="page-sub">{pk.writtenBy}</p>{/if}
					<div
						class={['summary-bar', problems ? 'fail' : pk.tamper ? 'fail' : 'pass']}
						role="status"
						aria-live="polite"
					>
						{#if problems}
							<span class="status fail"
								><Icon name="cross" size="glyph" />{problems}
								{problems === 1 ? 'problem' : 'problems'}</span
							>
						{:else if pk.tamper}
							<span class="status fail"
								><Icon name="cross" size="glyph" />Changed after signing</span
							>
						{:else}
							<span class="status pass"
								><Icon name={pk.signed ? 'stamp' : 'check'} size="glyph" />{pk.signed
									? 'Signed'
									: 'Valid'}</span
							>
						{/if}
						<span class="muted small">{pk.summary}</span>
					</div>

					{#if pk.tamper}
						<section class="section" aria-labelledby="tamper-h">
							<div class="section-head">
								<h2 class="section-title" id="tamper-h">What changed since it was signed</h2>
							</div>
							<Diff diff={pk.tamper} label="Diff of the signed packet against its commit" />
							<div class="actions top">
								<button class="btn primary" type="button" onclick={() => act('restore')}
									><Icon name="restore" />Restore the signed version</button
								>
							</div>
						</section>
					{/if}
					{#if pk.signed && !pk.signed.byForeman && !pk.signed.confirmed}
						<div class="actions top">
							<button class="btn" type="button" onclick={() => act('confirmSignature')}
								>Confirm I signed it</button
							>
							<span class="reason"
								>Claude writing a name never signs a packet. Rust asks you in a macOS dialog.</span
							>
						</div>
					{/if}

					<dl class="pfields" aria-label="Packet fields">
						{#each pk.fields as fd (fd.key)}
							<div
								class={[
									'pf',
									fd.problems.length && 'has-problem',
									pk.normalization.some((n) => n.field === fd.key) &&
										!fd.problems.length &&
										'has-note'
								]}
							>
								<dt>{fd.label}</dt>
								<dd>
									{#if fd.missing}<span class="muted">Missing</span>{:else}<Doc
											nodes={fd.value}
										/>{/if}
									{#if fd.note}<span class="muted"><Text spans={fd.note} /></span>{/if}
									{#if pk.normalization.some((n) => n.field === fd.key)}<button
											class="btn ghost xs norm"
											type="button"
											onclick={() => act('normalize')}>Normalize the file</button
										>{/if}
									{#each fd.problems as pr, i (i)}
										<div class="problem">
											<p><Icon name="cross" size="sm" /><span>{pr.message}</span></p>
											<div class="actions">
												{#if pr.fix?.includes('repair') && !isChat}
													<button class="btn" type="button" onclick={repair}
														><Icon name="wrench" />Repair</button
													>
												{/if}
												{#if isChat}
													<button class="btn" type="button" onclick={fixRequest}
														><Icon name="copy" />Copy fix request</button
													>
												{/if}
												{#if pr.fix?.includes('openInEditor')}
													<button
														class="btn ghost"
														type="button"
														aria-disabled={noEditor ? 'true' : undefined}
														aria-describedby={noEditor ? 'no-editor' : undefined}
														onclick={() =>
															noEditor
																? ui.say('Set an editor in Settings to open files.')
																: void openInEditor()}><Icon name="file" />Open in editor</button
													>
												{/if}
											</div>
										</div>
									{/each}
								</dd>
							</div>
						{/each}
					</dl>
					{#if noEditor}<p class="reason" id="no-editor">
							Open in editor needs an editor: set one in Settings.
						</p>{/if}
					{#if problems && !isChat}
						<p class="remap top">
							Repair re-arms the stage's session with: "Fix only these problems in {pk.path}: … Do
							not change anything else."
						</p>
					{/if}
				{/if}
			</div>
		</main>
		<Inspector {project} {stage} initial="packet" label="Inspector: raw packet and conditions">
			{#snippet packetPane()}
				{#if undecided.length}
					<div class="condition-box" role="group" aria-labelledby="cc-h">
						<h2 class="insp-title" id="cc-h">
							<Icon name="question" size="glyph" tone="attention" /> Confirm conditions
						</h2>
						<p class="insp-sub ink">
							{undecided.map(([id]) => id).join(', ')}
							{undecided.length === 1 ? 'is' : 'are'} undecided. Signing Stage 2 freezes the conditions,
							so decide first.
						</p>
						{#each undecided as [id, v] (id)}
							<div class="row">
								<code id="cc-{id}">{id}</code>
								<Segmented
									options={TRI}
									value={v.value}
									labelledby="cc-{id}"
									disabled={v.locked}
									onchange={(t: TriState) => setCondition(id, t)}
								/>
							</div>
						{/each}
					</div>
				{/if}
				<div>
					<h2 class="insp-title">Raw file</h2>
					<p class="insp-sub">{pk?.path ?? ''}{problems ? ', lines with problems marked' : ''}</p>
				</div>
				{#if pk?.raw.length}
					<!-- svelte-ignore a11y_no_noninteractive_tabindex (scrollable regions are focusable, components.md) -->
					<pre
						class="raw"
						tabindex="0"
						aria-label="Raw packet text">{#each pk.raw as r (r.line)}<span
								class={['rawline', r.bad && 'bad']}
								><span class="ln" aria-hidden="true">{r.line}</span><Text
									spans={r.spans}
								/>{#if r.bad}<span class="vh"> (problem)</span>{/if}</span
							>{/each}</pre>
				{:else}
					<p class="insp-sub">No file yet.</p>
				{/if}
			{/snippet}
		</Inspector>
	</div>
{/if}

<style>
	.narrow {
		max-width: 920px;
	}
	.packet-head {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		flex-wrap: wrap;
	}
	.packet-header-line {
		font-family: var(--font-mono);
		font-size: var(--text-mono);
		padding: 6px 10px;
		background: var(--well);
		border: var(--hairline) solid var(--rule);
	}
	.summary-bar {
		display: flex;
		gap: var(--space-5);
		align-items: center;
		padding: var(--space-3) var(--space-4);
		margin-top: var(--space-5);
		border: var(--hairline) solid var(--fail);
		background: var(--fail-wash);
	}
	.summary-bar.pass {
		border-color: var(--pass);
		background: var(--pass-wash);
	}
	.summary-bar .status {
		font-weight: var(--weight-semibold);
	}
	.small {
		font-size: var(--text-12);
	}
	.pfields {
		margin-top: var(--space-5);
		border-top: var(--hairline) solid var(--rule);
	}
	.pf {
		display: grid;
		grid-template-columns: 170px minmax(0, 1fr);
		gap: var(--space-5);
		padding: 10px var(--space-4);
		border-bottom: var(--hairline) solid var(--rule);
	}
	.pf dt {
		font-size: var(--text-12);
		color: var(--ink-muted);
		font-weight: var(--weight-medium);
		padding-top: 1px;
	}
	.pf dd {
		min-width: 0;
	}
	.pf :global(.doc-p) {
		display: inline;
	}
	.pf.has-problem {
		background: var(--fail-wash);
		box-shadow: inset 3px 0 0 var(--fail);
	}
	.pf.has-note {
		box-shadow: inset 3px 0 0 var(--accent-ink);
	}
	.problem {
		margin-top: var(--space-3);
		display: grid;
		gap: var(--space-3);
		font-size: var(--text-12);
	}
	.problem p {
		color: var(--ink);
		display: flex;
		gap: 6px;
		align-items: flex-start;
	}
	.problem p :global(.icon) {
		color: var(--fail);
		margin-top: 1px;
	}
	.problem .actions {
		gap: var(--space-2);
	}
	.top {
		margin-top: var(--space-4);
	}
	.raw {
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--text-11);
		line-height: 1.6;
		white-space: pre;
		overflow-x: auto;
	}
	.rawline {
		display: block;
	}
	.ln {
		color: var(--ink-muted);
		display: inline-block;
		width: 22px;
		text-align: right;
		margin-right: var(--space-3);
		user-select: none;
	}
	.bad {
		background: var(--fail-wash);
		color: var(--ink);
		box-shadow: inset 2px 0 0 var(--fail);
	}
	.condition-box {
		border: 1px solid var(--attention);
		background: var(--attention-wash);
		padding: var(--space-4);
		display: grid;
		gap: var(--space-3);
	}
	.condition-box .row {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		justify-content: space-between;
	}
	.norm {
		margin-left: var(--space-2);
	}
	.ink {
		color: var(--ink);
	}
</style>
