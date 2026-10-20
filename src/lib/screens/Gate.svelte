<script lang="ts">
	// Close out a stage (F-015, F-014, mockup gate.html): the close-out sequence, Gate A and Import
	// results, the inspection items in review order (failed, waiting, then passed collapsed), the attest
	// control, Gate B and C, and Sign. The sign sheet is the review; Sign packet only *requests*
	// signing and Rust raises the native confirmation (SR-24a). After read-back the stamp lands.
	import { report, runPrepared, runStep } from '#lib/actions/run.ts';
	import { failureText } from '#lib/actions/words.ts';
	import Dialog from '#lib/components/Dialog.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Inspector from '#lib/components/Inspector.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type {
		GateItemView,
		GateView,
		ImportProposal,
		ProjectView,
		SignPreview
	} from '#lib/ipc/types.ts';
	import Diff from '#lib/render/Diff.svelte';
	import Text from '#lib/render/Text.svelte';
	import { refresh } from '#lib/stores/bus.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	let { project, stageNo }: { project: ProjectView; stageNo: number } = $props();
	const pid = $derived(project.summary.id);
	const stage = $derived(project.stages.find((s) => s.stage === stageNo) ?? null);
	const gate = resource(
		() => ({ projectId: pid, stage: stageNo, rev: project.summary.rev }),
		(c, a) => c.gateView({ projectId: a.projectId, stage: a.stage }),
		(a) => [`project:${a.projectId}`]
	);
	const g: GateView | undefined = $derived(gate.value);
	$effect(() => {
		ui.setChrome({
			title: project.summary.name,
			sub: `Stage ${stageNo} close-out, branch ${project.branch}`,
			inspector: true
		});
	});

	const failed = $derived(g?.items.filter((i) => i.status === 'fail') ?? []);
	const waiting = $derived(g?.items.filter((i) => i.status === 'waiting') ?? []);
	const passed = $derived(
		g?.items.filter((i) => i.status === 'pass' || i.status === 'notApplicable') ?? []
	);
	let selectedId = $state<string | null>(null);
	const selected = $derived(
		g?.items.find((i) => i.id === selectedId) ?? failed[0] ?? waiting[0] ?? null
	);

	// Attest drafts per item.
	const drafts = $state<
		Record<string, { kind: 'done' | 'notApplicable'; note: string; error: string | null }>
	>({});
	$effect(() => {
		for (const it of g?.items ?? [])
			if (it.status === 'waiting' && !(it.id in drafts))
				drafts[it.id] = { kind: 'done', note: it.note ?? '', error: null };
	});
	function draft(it: GateItemView) {
		return drafts[it.id] ?? { kind: 'done' as const, note: '', error: null };
	}
	const NOTE_RULES: Record<string, string> = {
		'note-too-short':
			'Write at least 15 characters on how you checked, for example which ADRs you read.',
		'note-repeated': "That note repeats another item's. Say how you checked this one.",
		'kind-needs-reconfirm': 'The item text changed. Re-confirm its kind first.',
		'reason-required': 'Say why it doesn’t apply.'
	};
	function localError(it: GateItemView): string | null {
		const d = drafts[it.id];
		if (!d) return null;
		if (d.kind === 'done' && d.note.trim().length < 15) return NOTE_RULES['note-too-short'] ?? null;
		if (d.kind === 'notApplicable' && !d.note.trim()) return NOTE_RULES['reason-required'] ?? null;
		return null;
	}
	async function attest(it: GateItemView): Promise<void> {
		const d = draft(it);
		const local = localError(it);
		if (local) {
			d.error = local;
			return;
		}
		const c = await ipc();
		try {
			gate.set(
				await c.gateItemAttest({
					projectId: pid,
					stage: stageNo,
					item: it.id,
					outcome:
						d.kind === 'done'
							? { kind: 'done', note: d.note, link: null }
							: { kind: 'notApplicable', reason: d.note }
				})
			);
			d.error = null;
			ui.toast(d.kind === 'done' ? 'Done recorded' : 'Not applicable recorded');
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			d.error =
				f.detail?.kind === 'notAllowed'
					? (NOTE_RULES[f.detail.rule] ?? failureText(f))
					: failureText(f);
		}
	}
	async function reconfirm(it: GateItemView): Promise<void> {
		const c = await ipc();
		try {
			gate.set(await c.itemKindConfirm({ projectId: pid, item: it.id, textSha256: it.textSha256 }));
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	async function fix(it: GateItemView): Promise<void> {
		const c = await ipc();
		try {
			const prep = await c.gateFixStart({ projectId: pid, stage: stageNo, item: it.id });
			report(
				await runPrepared({ projectId: pid, step: '', surface: 'cli', entry: 'stageView' }, prep)
			);
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	const gateA = $derived(stage?.steps.find((s) => s.kind === 'gate') ?? null);
	let proposals = $state<ImportProposal[] | null>(null);
	async function importResults(): Promise<void> {
		const c = await ipc();
		try {
			proposals = await c.gateImportResults({ projectId: pid, stage: stageNo });
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	async function confirmProposal(p: ImportProposal): Promise<void> {
		if (p.proposed === 'unmatched') return;
		const c = await ipc();
		try {
			gate.set(
				await c.gateImportConfirm({
					projectId: pid,
					stage: stageNo,
					item: p.item,
					result: p.proposed,
					evidence: p.evidence.map((s) => ('text' in s ? s.text : '')).join('')
				})
			);
			proposals = proposals?.filter((x) => x.item !== p.item) ?? null;
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}

	// Signing (F-014).
	let signing = $state(false);
	let signName = $state('');
	let signDate = $state('2026-10-19');
	let branchKind = $state<'current' | 'other'>('current');
	let otherBranch = $state('');
	let sp = $state<SignPreview | null>(null);
	let signError = $state<string | null>(null);
	let progress = $state<string | null>(null);
	async function openSign(): Promise<void> {
		if (!g?.signable) {
			ui.say(signWhy);
			return;
		}
		const c = await ipc();
		const s = await c.settingsGet({});
		signName = s.driName ?? project.dri ?? '';
		signing = true;
		await previewSign();
	}
	async function previewSign(): Promise<void> {
		const c = await ipc();
		try {
			sp = await c.signPreview({
				projectId: pid,
				stage: stageNo,
				branch:
					branchKind === 'current' ? { kind: 'current' } : { kind: 'other', branch: otherBranch },
				name: signName,
				date: signDate
			});
			signError = null;
		} catch (e: unknown) {
			sp = null;
			signError = failureText(toIpcFailure(e));
		}
	}
	async function sign(): Promise<void> {
		if (!sp) return;
		const c = await ipc();
		let reads = 0;
		try {
			const r = await c.signApply({ previewId: sp.previewId }, (p) => {
				progress = p.step;
				if (p.step === 'readBack') reads += 1;
			});
			signing = false;
			progress = null;
			if (r.kind === 'signed') {
				ui.toast(
					`Packet signed. Committed ${r.commit} on ${sp.branch} and read back ${reads} files.`,
					'pass',
					'stamp'
				);
				setTimeout(() => (ui.stampStage = { projectId: pid, stage: stageNo }), 0);
				refresh(`project:${pid}`, 'today', 'projects');
			} else if (r.kind === 'commitPending')
				signError = `The commit didn't finish: ${r.stderrTail}`;
			else signError = 'Read-back found a difference. Nothing was signed.';
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			progress = null;
			if (f.code === 'cancelled') ui.say('Cancelled. Nothing was signed.');
			else
				signError =
					f.code === 'stale'
						? 'The packet changed since you reviewed it. Review it again.'
						: failureText(f);
		}
	}
	const signWhy = $derived(
		g?.signable
			? ''
			: g?.state === 'signed'
				? `Stage ${stageNo} is signed.`
				: `Sign is locked: ${g?.whyNot.join('. ') ?? ''}.`
	);
	const SEQ: Record<
		string,
		{
			icon: 'check' | 'cross' | 'ring' | 'lock' | 'dot';
			tone: 'pass' | 'fail' | 'muted' | 'accent';
		}
	> = {
		done: { icon: 'check', tone: 'pass' },
		failed: { icon: 'cross', tone: 'fail' },
		now: { icon: 'dot', tone: 'accent' },
		pending: { icon: 'ring', tone: 'muted' },
		locked: { icon: 'lock', tone: 'muted' }
	};
	const DONE_NA = [
		{ value: 'done' as const, label: 'Done' },
		{ value: 'notApplicable' as const, label: 'Not applicable' }
	];
</script>

{#snippet item(it: GateItemView)}
	{@const d = it.status === 'waiting' ? (drafts[it.id] ?? null) : null}
	<div
		class={['item', selected?.id === it.id && 'is-selected']}
		role="listitem"
		aria-current={selected?.id === it.id ? 'true' : undefined}
	>
		{#if it.status === 'fail'}
			<span class="stampmark fail" role="img" aria-label="Failed"
				><Icon name="cross" size="glyph" />FAIL</span
			>
		{:else if it.status === 'pass'}
			<span class="stampmark pass" role="img" aria-label="Passed"
				><Icon name="check" size="glyph" />PASS</span
			>
		{:else if it.status === 'notApplicable'}
			<span class="stampmark wait" role="img" aria-label="Not applicable">N/A</span>
		{:else}
			<span class="stampmark wait" role="img" aria-label="Waiting for you">Waiting</span>
		{/if}
		<div>
			<button class="item-title" type="button" onclick={() => (selectedId = it.id)}
				>{it.text}</button
			>
			{#if it.how}<p class="item-how">{it.how}</p>{/if}
			{#if it.evidence}
				<div class={['evidence', it.proposed === 'pass' && 'proposed']}>
					<Text spans={it.evidence} />
				</div>
			{/if}
			{#if it.needsReconfirm}
				<div class="actions">
					<button class="btn" type="button" onclick={() => reconfirm(it)}>Re-confirm kind</button
					><span class="reason">The item text changed since the blueprint pin.</span>
				</div>
				<dl class="drift">
					<dt>Pinned</dt>
					<dd>{it.previousText ?? 'The pinned text is no longer in the blueprint.'}</dd>
					<dt>Now</dt>
					<dd>{it.text}</dd>
				</dl>
			{:else if it.status === 'fail'}
				<div class="actions">
					<button class="btn primary" type="button" onclick={() => fix(it)}
						><Icon name="wrench" />Fix this</button
					>
					<span class="reason"
						>Opens a new CLI chain: Opus 5.5, high, Plan, with this item and its evidence</span
					>
				</div>
			{:else if d}
				<div class="attest" role="group" aria-label="Attest {it.id}">
					<Segmented
						options={DONE_NA}
						value={d.kind}
						label="Result"
						onchange={(v: 'done' | 'notApplicable') => (d.kind = v)}
					/>
					<div class="field">
						<label for="n-{it.id}"
							>{d.kind === 'done' ? 'How you checked' : 'Why it doesn’t apply'}</label
						>
						<textarea
							class="textarea"
							id="n-{it.id}"
							rows="2"
							bind:value={d.note}
							aria-invalid={d.error ? 'true' : undefined}
							aria-describedby="n-{it.id}-msg"></textarea>
						{#if d.error}
							<p class="err" id="n-{it.id}-msg">
								<Icon name="cross" size="glyph" /><span>{d.error}</span>
							</p>
						{:else}
							<span class="counter" id="n-{it.id}-msg"
								>{d.note.length} characters. A note needs at least 15 and can't repeat another item's.</span
							>
						{/if}
					</div>
					<div class="actions">
						<button class="btn" type="button" onclick={() => attest(it)}
							><Icon name="check" />Confirm {d.kind === 'done' ? 'Done' : 'Not applicable'}</button
						>
					</div>
				</div>
			{/if}
		</div>
	</div>
{/snippet}

{#if stage}
	<div class="body">
		<main class="main" id="main" aria-labelledby="gate-title">
			<div class="page wide">
				<h1 class="page-title" id="gate-title">Close out {stage.title}</h1>
				{#if gate.error}
					<p class="err" role="alert">
						<Icon name="cross" size="glyph" />{failureText(gate.error)}
					</p>
				{:else if g}
					<ol class="sequence" aria-label="Close-out sequence">
						{#each g.closeout as s (s.key)}
							{@const l = SEQ[s.state] ?? SEQ.pending}
							<li
								class={[
									s.state === 'done' && 'done',
									(s.state === 'now' || s.state === 'failed') && 'now',
									s.state === 'failed' && 'failed'
								]}
								aria-current={s.state === 'now' || s.state === 'failed' ? 'step' : undefined}
							>
								<Icon name={l?.icon ?? 'ring'} size="glyph" tone={l?.tone ?? 'muted'} />{s.label}
							</li>
						{/each}
					</ol>
					<div class="gate-top">
						{#if gateA}
							<button
								class="btn"
								type="button"
								onclick={() =>
									runStep({
										projectId: pid,
										step: gateA.key,
										surface: 'cli',
										entry: 'stageView'
									}).then(report)}><Icon name="terminal" />Run Gate A in terminal</button
							>
						{/if}
						<button class="btn" type="button" onclick={importResults}
							><Icon name="import" />Import results</button
						>
						{#if g.importedFrom}<span class="reason">{g.importedFrom}</span>{/if}
					</div>

					<div class="items" role="list" aria-label="Inspection items, failed first">
						{#each [...failed, ...waiting] as it (it.id)}{@render item(it)}{/each}
					</div>
					{#if passed.length}
						<details class="passed">
							<summary
								><Icon name="chev" class="chev" />{passed.length} passed{passed.length === 1
									? `: ${passed[0]?.text.charAt(0).toLowerCase()}${passed[0]?.text.slice(1)}`
									: ''}</summary
							>
							<div role="list" aria-label="Passed items">
								{#each passed as it (it.id)}{@render item(it)}{/each}
							</div>
						</details>
					{/if}

					<div class="closeout" aria-label="After the inspection">
						{#each g.closeout.filter((s) => s.key === 'gateB' || s.key === 'gateC') as s (s.key)}
							<div class="closeout-row">
								<Icon
									name={s.state === 'done' ? 'check' : 'lock'}
									size="glyph"
									tone={s.state === 'done' ? 'pass' : 'muted'}
								/><span>{s.label}</span><span class="muted"
									>{(s.note ?? '').replace('0N', String(stageNo).padStart(2, '0'))}</span
								>
							</div>
						{/each}
					</div>
					<div class="sign-row">
						<button
							class="btn primary lg"
							type="button"
							aria-disabled={g.signable ? undefined : 'true'}
							aria-describedby={g.signable ? undefined : 'sign-why'}
							onclick={openSign}
							><Icon name="pen" />{g.state === 'signed' ? 'Packet signed' : 'Sign packet'}</button
						>
						{#if !g.signable}<span class="reason" id="sign-why">{signWhy}</span>{/if}
					</div>
				{/if}
			</div>
		</main>
		<Inspector {project} {stage} initial="gate" label="Inspector: evidence">
			{#snippet gatePane()}
				{#if selected}
					<div>
						<h2 class="insp-title">
							Evidence for {selected.id
								.split('-')
								.map((w) => (w.length <= 3 ? w.toUpperCase() : w))
								.join(' ')}
						</h2>
						<p class="insp-sub">
							{selected.session
								? "From the Gate A session's last message"
								: 'From your attestation'}
						</p>
					</div>
					{#if selected.session}
						<dl class="kv">
							<dt>Session</dt>
							<dd><code>{selected.session}</code></dd>
							<dt>Model</dt>
							<dd>Opus 5.5, high, Plan</dd>
							<dt>Agents</dt>
							<dd>test-runner, security-reviewer</dd>
						</dl>
					{/if}
					{#if selected.artifacts.length}
						<div>
							<h3 class="insp-title small">Artifacts</h3>
							<ul class="insp-list">
								{#each selected.artifacts as a (a.text)}
									<li>
										<Icon
											name={a.kind === 'path' ? 'file' : 'terminal'}
											size="glyph"
											tone="muted"
										/>
										<span
											><code><Text text={a.text} /></code><span class="kind"
												><button
													class="btn ghost xs"
													type="button"
													onclick={() =>
														ui.say(
															'Copy needs the clipboard command (ipc.md clipboard_copy has no artifact target yet).'
														)}><Icon name="copy" />Copy {a.kind}</button
												></span
											></span
										>
									</li>
								{/each}
							</ul>
						</div>
					{/if}
					{#if selected.failures >= 2}<p class="insp-sub">
							Second failure of this item. On a third, Foreman offers the F.4 prompt.
						</p>{/if}
				{:else}
					<div class="empty">
						<strong>No item selected</strong><span>Choose an item to see its evidence.</span>
					</div>
				{/if}
			{/snippet}
		</Inspector>
	</div>
{/if}

{#if proposals}
	<Dialog
		title="Import results from Gate A"
		description="Proposals from the session's last message. Nothing is recorded until you confirm it."
		onclose={() => (proposals = null)}
	>
		<ul class="writes">
			{#each proposals as p (p.item)}
				<li>
					<span class="what"
						>{p.proposed === 'unmatched' ? 'No match' : p.proposed.toUpperCase()}</span
					>
					<span
						><b>{p.item}</b>
						<Text spans={p.evidence} />
						{#if p.proposed !== 'unmatched'}<button
								class="btn xs"
								type="button"
								onclick={() => confirmProposal(p)}>Confirm</button
							>{/if}</span
					>
				</li>
			{:else}
				<li><span class="what">Done</span><span>Every proposal is confirmed.</span></li>
			{/each}
		</ul>
		{#snippet footer()}
			<span class="spacer"></span>
			<button class="btn" type="button" onclick={() => (proposals = null)}>Close</button>
		{/snippet}
	</Dialog>
{/if}

{#if signing}
	<Dialog
		title="Sign the Stage {stageNo} packet"
		description={sp
			? `Signing ${sp.paths[0] ?? ''} @ ${sp.short}. Foreman re-reads the file first and refuses if it changed.`
			: 'Preparing the signature…'}
		onclose={() => (signing = false)}
	>
		<div class="two">
			<div class="field">
				<label for="s-name">Name</label><input
					class="input"
					id="s-name"
					bind:value={signName}
					onchange={previewSign}
				/>
			</div>
			<div class="field">
				<label for="s-date">Date</label><input
					class="input"
					id="s-date"
					bind:value={signDate}
					onchange={previewSign}
				/>
			</div>
		</div>
		<div class="field">
			<span class="field-label" id="branch-l">Branch</span>
			<div class="radio-list" role="radiogroup" aria-labelledby="branch-l">
				{#each [{ v: 'current' as const, label: 'Current branch,', code: project.branch }, { v: 'other' as const, label: 'Sign on another branch (leaves your working tree untouched)', code: '' }] as o, i (o.v)}
					<button
						type="button"
						class="radio"
						role="radio"
						aria-checked={branchKind === o.v}
						tabindex={branchKind === o.v ? 0 : -1}
						onclick={() => {
							branchKind = o.v;
							void previewSign();
						}}
						onkeydown={(e) => {
							if (
								e.key !== 'ArrowDown' &&
								e.key !== 'ArrowUp' &&
								e.key !== 'ArrowLeft' &&
								e.key !== 'ArrowRight'
							)
								return;
							e.preventDefault();
							branchKind = i === 0 ? 'other' : 'current';
							void previewSign();
							const host = e.currentTarget.parentElement;
							queueMicrotask(() =>
								host?.querySelectorAll<HTMLElement>('[role="radio"]')[i === 0 ? 1 : 0]?.focus()
							);
						}}
						><span class={['r', branchKind === o.v && 'on']} aria-hidden="true"
						></span>{o.label}{#if o.code}&nbsp;<code>{o.code}</code>{/if}</button
					>
				{/each}
			</div>
			{#if branchKind === 'other'}
				<label class="vh" for="s-branch">Branch name</label>
				<input
					class="input mono"
					id="s-branch"
					bind:value={otherBranch}
					placeholder="handoff/stage-{String(stageNo).padStart(2, '0')}"
					onchange={previewSign}
				/>
				<p class="hint">Signing on another branch leaves your working tree untouched.</p>
			{/if}
		</div>
		{#if sp}
			<div class="field">
				<span class="field-label">Fields Foreman will add</span>
				<Diff diff={sp.addedFields} label="Diff of the fields Foreman will add" />
			</div>
			<div class="field">
				<span class="field-label"
					>Commits only these paths, with repo hooks off, then reads each one back</span
				>
				<ul class="writes">
					{#each sp.paths as path, i (path)}
						<li><span class="what">{i === 0 ? 'Packet' : 'Record'}</span><code>{path}</code></li>
					{/each}
				</ul>
			</div>
			{#if sp.confirmConditions?.length}<p class="reason attention">
					<Icon name="question" size="glyph" />Signing freezes the conditions: {sp.confirmConditions.join(
						', '
					)}.
				</p>{/if}
		{/if}
		{#if signError}<p class="err" role="alert">
				<Icon name="cross" size="glyph" />{signError}
			</p>{/if}
		{#if progress}<p class="reason" role="status">Signing: {progress}</p>{/if}
		{#snippet footer()}
			<span class="hashline"
				>Commit message: {sp?.commitMessage ?? ''} · Sign packet asks you once more in a macOS dialog</span
			>
			<span class="spacer"></span>
			<button class="btn" type="button" onclick={() => (signing = false)}>Cancel</button>
			<button class="btn primary" type="button" disabled={!sp} onclick={sign}
				><Icon name="pen" />Sign packet</button
			>
		{/snippet}
	</Dialog>
{/if}

<style>
	.wide {
		max-width: none;
	}
	.sequence {
		display: flex;
		align-items: center;
		margin: var(--space-4) 0 var(--space-2);
		flex-wrap: wrap;
	}
	.sequence li {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.sequence li + li::before {
		content: '';
		width: 28px;
		height: 1px;
		background: var(--rule);
		margin: 0 10px;
	}
	.sequence li.done {
		color: var(--ink);
	}
	.sequence li.now {
		color: var(--ink);
		font-weight: var(--weight-semibold);
	}
	.sequence li.failed {
		color: var(--fail);
	}
	.gate-top {
		display: flex;
		gap: var(--space-3);
		align-items: center;
		flex-wrap: wrap;
		margin-top: var(--space-4);
	}
	.items {
		margin-top: var(--space-5);
		border-top: var(--hairline) solid var(--rule);
	}
	.item {
		display: grid;
		grid-template-columns: 92px minmax(0, 1fr);
		gap: var(--space-4);
		padding: var(--space-4) var(--space-3) var(--space-4) 0;
		border-bottom: var(--hairline) solid var(--rule);
	}
	.item.is-selected {
		background: var(--sheet-raised);
	}
	.stampmark {
		justify-self: center;
		display: grid;
		justify-items: center;
		gap: var(--space-1);
		font-family: var(--font-drafting);
		font-weight: var(--drafting-weight);
		font-stretch: var(--drafting-stretch);
		font-size: var(--text-12);
		letter-spacing: 0.04em;
		padding: 3px var(--space-3);
		border: var(--rule-strong) solid currentColor;
		border-radius: var(--radius-pill);
		transform: rotate(-4deg);
		align-self: start;
		margin-top: var(--space-1);
	}
	.stampmark.fail {
		color: var(--fail);
	}
	.stampmark.pass {
		color: var(--pass);
	}
	.stampmark.wait {
		color: var(--ink-muted);
		border-style: dashed;
		transform: none;
	}
	.item-title {
		all: unset;
		font-weight: var(--weight-medium);
		cursor: default;
	}
	.item-title:focus-visible {
		outline: var(--focus-width) solid var(--focus);
		outline-offset: var(--focus-offset);
	}
	.drift {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr);
		gap: var(--space-1) var(--space-3);
		margin: var(--space-2) 0 0;
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.drift dt {
		font-weight: var(--weight-semibold);
	}
	.drift dd {
		margin: 0;
		color: var(--ink);
	}
	.item-how {
		font-size: var(--text-12);
		color: var(--ink-muted);
		margin-top: var(--space-1);
	}
	.evidence {
		margin-top: var(--space-3);
		font-size: var(--text-12);
		padding: var(--space-3) var(--space-4);
		background: var(--well);
		border-left: 2px solid var(--fail);
	}
	.evidence.proposed {
		border-left-color: var(--pass);
	}
	.attest {
		margin-top: var(--space-3);
		display: grid;
		gap: var(--space-3);
		max-width: 640px;
	}
	.item .actions {
		margin-top: var(--space-3);
	}
	details.passed > summary {
		list-style: none;
		display: flex;
		gap: var(--space-3);
		align-items: center;
		padding: var(--space-3) 0;
		color: var(--ink-muted);
		font-size: var(--text-12);
		border-bottom: var(--hairline) solid var(--rule);
	}
	details.passed > summary::-webkit-details-marker {
		display: none;
	}
	.closeout {
		margin-top: var(--space-6);
		display: grid;
		gap: var(--space-3);
	}
	.closeout-row {
		display: grid;
		grid-template-columns: 20px 150px minmax(0, 1fr);
		gap: var(--space-3);
		align-items: center;
		font-size: var(--text-13);
		padding: 6px 0;
		border-bottom: var(--hairline) solid var(--rule);
	}
	.sign-row {
		display: flex;
		gap: var(--space-4);
		align-items: center;
		margin-top: var(--space-4);
	}
	.two {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-4);
	}
	.radio-list {
		display: grid;
		gap: 6px;
	}
	.radio {
		all: unset;
		display: flex;
		gap: var(--space-3);
		align-items: center;
		font-size: var(--text-13);
	}
	.radio:focus-visible {
		outline: var(--focus-width) solid var(--focus);
		outline-offset: var(--focus-offset);
	}
	.r {
		width: 14px;
		height: 14px;
		border-radius: 50%;
		border: 1px solid var(--control-edge);
		display: grid;
		place-items: center;
		background: var(--sheet);
	}
	.r.on {
		border-color: var(--accent-ink);
	}
	.r.on::after {
		content: '';
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--accent-ink);
	}
	.hashline {
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.small {
		font-size: var(--text-12);
	}
</style>
