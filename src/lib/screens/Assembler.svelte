<script lang="ts">
	// Build-package assembler (F-023 P2, mockup assembler.html): the Stage 4 packet's REPO FILES with
	// their state by glyph and word, Paste into (clipboard read in Rust, then a diff sheet) and Reveal;
	// late conditions with evidence proposals that you confirm; and "The package is complete" once every
	// row Foreman may write exists. Root CLAUDE.md asks again in a native dialog (AC-F021-06).
	import { failureText } from '#lib/actions/words.ts';
	import Dialog from '#lib/components/Dialog.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type {
		AssemblerRow,
		ConditionId,
		PastePreview,
		ProjectView,
		TriState
	} from '#lib/ipc/types.ts';
	import { to } from '#lib/nav/routes.ts';
	import Diff from '#lib/render/Diff.svelte';
	import Text from '#lib/render/Text.svelte';
	import { refresh } from '#lib/stores/bus.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	let { project }: { project: ProjectView } = $props();
	const pid = $derived(project.summary.id);
	$effect(() => {
		ui.setChrome({ title: project.summary.name, sub: 'Stage 4 · Build package' });
	});
	const asm = resource(
		() => ({ projectId: pid, rev: project.summary.rev }),
		(c, a) => c.assemblerView({ projectId: a.projectId }),
		(a) => [`project:${a.projectId}`]
	);
	const a = $derived(asm.value);
	const counts = $derived({
		ready: a?.rows.filter((r) => r.state === 'exists').length ?? 0,
		missing: a?.rows.filter((r) => r.state === 'missing').length ?? 0,
		empty: a?.rows.filter((r) => r.state === 'empty').length ?? 0,
		refused: a?.rows.filter((r) => r.state === 'refused').length ?? 0,
		total: a?.rows.length ?? 0
	});
	const LOOK: Record<
		AssemblerRow['state'],
		{
			icon: 'check' | 'ring' | 'warn' | 'cross';
			tone: 'pass' | 'muted' | 'attention' | 'fail';
			word: string;
		}
	> = {
		exists: { icon: 'check', tone: 'pass', word: 'Exists' },
		missing: { icon: 'ring', tone: 'muted', word: 'Missing' },
		empty: { icon: 'warn', tone: 'attention', word: 'Empty' },
		refused: { icon: 'cross', tone: 'fail', word: 'Refused' }
	};
	let sheet = $state<{ row: AssemblerRow; preview: PastePreview } | null>(null);
	let returnTo: HTMLElement | null = null;

	async function pasteInto(row: AssemblerRow, e: MouseEvent): Promise<void> {
		returnTo = e.currentTarget as HTMLElement;
		const c = await ipc();
		try {
			sheet = { row, preview: await c.assemblerPastePreview({ projectId: pid, path: row.path }) };
		} catch (err: unknown) {
			ui.say(failureText(toIpcFailure(err)));
		}
	}
	async function write(): Promise<void> {
		const s = sheet;
		if (!s) return;
		const c = await ipc();
		try {
			asm.set(await c.assemblerPasteApply({ previewId: s.preview.previewId }));
			ui.toast(`Wrote ${s.row.path}`);
			sheet = null;
			returnTo?.focus();
			refresh(`project:${pid}`);
		} catch (err: unknown) {
			const f = toIpcFailure(err);
			ui.say(f.code === 'cancelled' ? `Cancelled. ${s.row.path} is unchanged.` : failureText(f));
		}
	}
	async function reveal(row: AssemblerRow): Promise<void> {
		const c = await ipc();
		try {
			await c.fileReveal({ projectId: pid, path: row.path });
		} catch (err: unknown) {
			ui.say(failureText(toIpcFailure(err)));
		}
	}
	const chosen = $state<Partial<Record<ConditionId, TriState>>>({});
	async function confirm(id: ConditionId, value: TriState, evidence: string | null): Promise<void> {
		const c = await ipc();
		try {
			await c.conditionSet({
				projectId: pid,
				condition: id,
				to: value,
				note: evidence ? `from evidence: ${evidence}` : 'decided at the build package'
			});
			ui.toast(`${id} = ${value} recorded`);
			refresh(`project:${pid}`);
			await asm.reload();
		} catch (err: unknown) {
			ui.say(failureText(toIpcFailure(err)));
		}
	}
	const TRI: { value: TriState; label: string }[] = [
		{ value: 'yes', label: 'Yes' },
		{ value: 'no', label: 'No' },
		{ value: 'undecided', label: 'Undecided' }
	];
	const blocking = $derived(
		a?.rows.filter((r) => r.state === 'missing' || r.state === 'empty') ?? []
	);
</script>

<div class="body no-inspector">
	<main class="main" id="main" aria-labelledby="asm-title">
		<div class="page narrow">
			<h1 class="page-title" id="asm-title">Build package</h1>
			{#if asm.error}
				<div class="empty">
					<strong>No build package yet</strong>
					<span
						>The assembler reads the REPO FILES list from a valid Stage 4 packet. Run 4.8 first; its
						packet lands in docs/handoff/stage-04.md.</span
					>
					<a class="btn" href={to.stage(pid, 4)}>Open Stage 4</a>
				</div>
			{:else if a}
				<p class="page-sub">
					From the REPO FILES list in <code>docs/handoff/stage-04.md</code>. Paste each file from
					your Stage 4 chats; Foreman writes only what the path policy allows and shows every
					overwrite first.
				</p>
				<div class={['asm-summary', a.complete && 'ok']} role="status" aria-live="polite">
					<span class="status"
						><Icon
							name={a.complete ? 'check' : 'warn'}
							size="glyph"
							tone={a.complete ? 'pass' : 'attention'}
						/>{counts.ready} of {counts.total} files ready</span
					>
					<span class="muted small"
						>{[
							counts.missing && `${counts.missing} missing`,
							counts.empty && `${counts.empty} empty`,
							counts.refused && `${counts.refused} refused`
						]
							.filter(Boolean)
							.join(', ')}. "The package is complete" is offered when every row Foreman may write
						exists and isn't empty.</span
					>
				</div>

				<section class="section" aria-labelledby="rf-title">
					<div class="section-head">
						<h2 class="section-title" id="rf-title">Repo files</h2>
						<span class="muted">in packet order</span>
					</div>
					<ul class="asm-rows" aria-label="Repo files from the Stage 4 packet">
						{#each a.rows as r (r.path)}
							{@const l = LOOK[r.state]}
							<li
								class={[
									'asm-row',
									r.state === 'refused' && 'is-refused',
									r.state === 'empty' && 'is-empty'
								]}
							>
								<span class={['status', r.state === 'refused' && 'fail']}
									><Icon name={l.icon} size="glyph" tone={l.tone} />{l.word}</span
								>
								<code><Text text={r.path} layout={false} /></code>
								<span class="actions">
									{#if r.state !== 'refused'}
										<button
											class={['btn', r.state === 'exists' && 'ghost']}
											type="button"
											onclick={(e) => pasteInto(r, e)}
											aria-label="Paste into {r.path}"><Icon name="paste" />Paste into</button
										>
									{/if}
									{#if r.state === 'exists' || r.state === 'empty'}
										<button
											class="btn ghost"
											type="button"
											onclick={() => reveal(r)}
											aria-label="Reveal {r.path} in Finder"><Icon name="folder" />Reveal</button
										>
									{/if}
								</span>
								{#if r.note}
									<span class="note"
										>{#if r.path === 'CLAUDE.md'}<Icon name="lock" size="sm" />{/if}
										{r.note}</span
									>
								{/if}
							</li>
						{/each}
					</ul>
				</section>

				{#if a.lateConditions.length}
					<section class="section" aria-labelledby="late-title">
						<div class="section-head">
							<h2 class="section-title" id="late-title">Late conditions</h2>
							<span class="muted"
								>decide before Stage 4 is signed; proposals come from file paths only</span
							>
						</div>
						<div class="late">
							{#each a.lateConditions as lc (lc.condition)}
								{@const proposed = lc.value === 'undecided' && lc.proposed !== null}
								{@const value = chosen[lc.condition] ?? (proposed ? lc.proposed : lc.value)}
								<div class={['late-row', proposed && 'is-proposed']}>
									<code id="lc-{lc.condition}">{lc.condition}</code>
									<Segmented
										options={TRI}
										{value}
										labelledby="lc-{lc.condition}"
										onchange={(v: TriState) => (chosen[lc.condition] = v)}
									/>
									<span class="evidence">
										{#if lc.decided}{lc.decided}{:else if proposed && lc.evidence}Proposed from: <code
												>{lc.evidence}</code
											>. Not recorded until you confirm.{:else}No evidence in REPO FILES or the
											repo. Decide it yourself.{/if}
									</span>
									{#if (proposed || chosen[lc.condition] !== undefined) && value}
										<button
											class="btn"
											type="button"
											onclick={() => confirm(lc.condition, value, proposed ? lc.evidence : null)}
											>Confirm</button
										>
									{:else}<span></span>{/if}
								</div>
							{/each}
						</div>
					</section>
				{/if}

				<section class="section" aria-labelledby="done-title">
					<div class="section-head">
						<h2 class="section-title" id="done-title">Package complete</h2>
					</div>
					<div class="complete-box">
						{#if a.complete}
							<p>
								<Icon name="check" tone="pass" /> <strong>The package is complete.</strong> Attest it
								in the Stage 4 gate.
							</p>
						{:else}
							<p>
								<Icon name="lock" /> <strong>The package is complete</strong> isn't offered yet.
							</p>
							<p class="muted small">
								Waiting for:
								{#each blocking as b, i (b.path)}<code>{b.path}</code> ({b.state}){i <
									blocking.length - 1
										? ', '
										: '.'}{/each}
								{#if counts.refused}
									The refused row doesn't block it: Foreman can never write it, so you create it
									yourself.{/if}
							</p>
						{/if}
					</div>
				</section>
			{/if}
		</div>
	</main>
</div>

{#if sheet}
	<Dialog
		title="Paste into {sheet.row.path}"
		description={sheet.row.state === 'missing'
			? 'Creates the file from the clipboard.'
			: 'Overwrites the file with the clipboard. The diff shows the new content.'}
		onclose={() => {
			sheet = null;
			returnTo?.focus();
		}}
	>
		{#each sheet.preview.targets as t (t.path)}
			<Diff diff={t} label="New content of {t.path}" />
		{/each}
		{#each sheet.preview.notes as n, i (i)}<p class="reason">{n}</p>{/each}
		{#if sheet.row.path === 'CLAUDE.md'}<p class="reason attention">
				<Icon name="lock" size="glyph" />Claude Code loads this file in every session. Writing it
				asks you again in a macOS dialog.
			</p>{/if}
		{#snippet footer()}
			<span class="spacer"></span>
			<button class="btn" type="button" onclick={() => (sheet = null)}>Cancel</button>
			<button class="btn primary" type="button" onclick={write}>Write file</button>
		{/snippet}
	</Dialog>
{/if}

<style>
	.narrow {
		max-width: 960px;
	}
	.small {
		font-size: var(--text-12);
	}
	.asm-summary {
		display: flex;
		gap: var(--space-5);
		align-items: center;
		padding: var(--space-3) var(--space-4);
		margin-top: var(--space-5);
		border: var(--hairline) solid var(--attention);
		background: var(--attention-wash);
	}
	.asm-summary.ok {
		border-color: var(--pass);
		background: var(--pass-wash);
	}
	.asm-summary .status {
		font-weight: var(--weight-semibold);
	}
	.asm-rows {
		margin-top: var(--space-4);
		border-top: var(--hairline) solid var(--rule);
	}
	.asm-row {
		display: grid;
		grid-template-columns: 120px minmax(0, 1fr) auto;
		gap: var(--space-4);
		align-items: center;
		padding: 9px var(--space-4);
		border-bottom: var(--hairline) solid var(--rule);
	}
	.asm-row code {
		font-family: var(--font-mono);
		font-size: var(--text-mono);
		overflow-wrap: anywhere;
	}
	.asm-row .note {
		grid-column: 2 / 4;
		font-size: var(--text-12);
		color: var(--ink-muted);
		margin-top: -4px;
	}
	.asm-row.is-refused {
		background: var(--fail-wash);
		box-shadow: inset 3px 0 0 var(--fail);
	}
	.asm-row.is-empty {
		box-shadow: inset 3px 0 0 var(--attention);
	}
	.asm-row .actions {
		display: flex;
		gap: var(--space-2);
	}
	.late {
		margin-top: var(--space-4);
		display: grid;
		border-top: var(--hairline) solid var(--rule);
	}
	.late-row {
		display: grid;
		grid-template-columns: 170px auto minmax(0, 1fr) auto;
		gap: var(--space-4);
		align-items: center;
		padding: 10px var(--space-4);
		border-bottom: var(--hairline) solid var(--rule);
	}
	.late-row code {
		font-family: var(--font-mono);
		font-size: var(--text-mono);
	}
	.late-row .evidence {
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.late-row.is-proposed {
		box-shadow: inset 3px 0 0 var(--accent-ink);
	}
	.complete-box {
		margin-top: var(--space-4);
		border: 1px dashed var(--control-edge);
		padding: var(--space-4);
		display: grid;
		gap: var(--space-2);
	}
</style>
