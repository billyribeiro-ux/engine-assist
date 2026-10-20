<script lang="ts">
	// Paste-back (components.md, F-012, ⌘⇧V): Rust reads the clipboard, validates and previews the
	// exact writes; nothing is written until Write. Errors stay in place; the region is a polite live
	// region so the result is announced. Pasted text is shown with visible escapes (SR-26).
	import { failureText } from '#lib/actions/words.ts';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { PastePreview } from '#lib/ipc/types.ts';
	import Diff from '#lib/render/Diff.svelte';
	import Text from '#lib/render/Text.svelte';
	import { refresh } from '#lib/stores/bus.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';
	import { tick } from 'svelte';

	import Icon from './Icon.svelte';

	let {
		projectId,
		step,
		stage,
		packet,
		api
	}: {
		projectId: string;
		step: string;
		stage: number;
		packet: boolean;
		/** Hands the parent the ⌘⇧V action. */
		api?: (trigger: (() => Promise<true | string>) | null) => void;
	} = $props();

	let preview = $state<PastePreview | null>(null);
	let error = $state<string | null>(null);
	let busy = $state(false);
	/** The written packet has problems: offer Copy fix request (AC-F012-11). */
	let fixable = $state(false);
	let writeBtn = $state<HTMLButtonElement>();
	const headId = $derived(`pb-${step.replace(/[^a-z0-9]/gi, '-')}`);

	// Refusals whose Rust message is the exact sentence to show (AC-F012-10, -14).
	const OWN_WORDS = new Set([
		'wrong-stage-packet',
		'two-packets',
		'appends-in-p1',
		'no-packet',
		'not-a-packet-step',
		'no-output'
	]);

	$effect(() => {
		api?.(trigger);
		return () => api?.(null);
	});

	async function trigger(): Promise<true | string> {
		const c = await ipc();
		error = null;
		fixable = false;
		try {
			preview = await c.pasteBackPreview({ projectId, step, kind: 'auto', source: 'clipboard' });
			// The exact writes are the next decision: move focus to Write so the keyboard can act.
			await tick();
			writeBtn?.focus();
			return true;
		} catch (e: unknown) {
			preview = null;
			const f = toIpcFailure(e);
			const d = f.detail;
			error =
				d?.kind === 'clipboard' && d.why === 'empty'
					? 'Clipboard is empty'
					: d?.kind === 'notAllowed' && OWN_WORDS.has(d.rule) && f.message
						? f.message
						: failureText(f);
			return true;
		}
	}

	async function write(): Promise<void> {
		if (!preview) return;
		const c = await ipc();
		busy = true;
		const invalid = preview.problems.length > 0 && preview.packetFields !== null;
		try {
			const r = await c.pasteBackApply({ previewId: preview.previewId });
			preview = null;
			fixable = invalid && !r.stepDone;
			ui.toast(
				`Wrote ${r.written.length} ${r.written.length === 1 ? 'file' : 'files'}${r.stepDone ? '; step done' : fixable ? '; the packet still has problems' : ''}.`
			);
			refresh(`project:${projectId}`, `packet:${projectId}:${stage}`, 'today');
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			error = f.code === 'expired' ? 'That preview expired. Paste back again.' : failureText(f);
			preview = null;
		} finally {
			busy = false;
		}
	}

	async function copyFix(): Promise<void> {
		const c = await ipc();
		try {
			await c.clipboardCopy({
				projectId,
				target: 'fixRequest',
				step: null,
				chain: null,
				path: null,
				stage
			});
			ui.toast('Fix request copied. Paste it into the chat.', 'pass', 'copy');
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}

	const creates = $derived(preview?.targets.filter((t) => !t.exists).length ?? 0);
	const total = $derived(preview?.targets.length ?? 0);
	const lines = (t: PastePreview['targets'][number]) =>
		t.hunks.reduce((n, h) => n + h.lines.length, 0);
</script>

<div
	class={['pasteback', preview && 'found', error && 'error']}
	role="region"
	aria-labelledby={headId}
	aria-live="polite"
>
	{#if preview}
		<div class="head">
			<h3 id={headId} class="insp-title">
				<Icon name="check" size="glyph" tone="pass" />
				{#if preview.packetFields}
					Packet found on the clipboard: Stage {preview.packetFields.fromStage}{preview.packetFields
						.toStage !== null
						? ` → Stage ${preview.packetFields.toStage}`
						: ''}
				{:else}
					Output found on the clipboard
				{/if}
			</h3>
			<kbd>Paste back ⌘⇧V</kbd>
		</div>
		<ul class="writes">
			{#each preview.targets as t, i (t.path)}
				<li>
					<span class="what">{t.exists ? 'Overwrite' : 'Create'}</span>
					<span
						><code><Text text={t.path} layout={false} /></code>
						{#if i === 1 && preview.packetFields}<span class="muted"
								>press release and FAQ, {lines(t)} lines found after the packet</span
							>{/if}
						{#if t.exists}<details class="pb-diff">
								<summary>Show what changes</summary>
								<Diff diff={t} label="Changes to {t.path}" />
							</details>{/if}</span
					>
				</li>
			{/each}
			{#if preview.notes.length}
				<li><span class="what">Normalized</span><span>{preview.notes.join('; ')}</span></li>
			{/if}
			{#each preview.problems as p, i (i)}
				<li>
					<span class="what err">Problem</span><span
						>{#if p.line !== null}<span class="muted">line {p.line}:</span>
						{/if}<Text text={p.message} layout={false} /></span
					>
				</li>
			{/each}
		</ul>
		<div class="actions">
			<button class="btn primary" type="button" disabled={busy} onclick={write} bind:this={writeBtn}
				>Write {total} {total === 1 ? 'file' : 'files'}</button
			>
			<button class="btn ghost" type="button" onclick={() => (preview = null)}>Cancel</button>
			{#if creates < total}<span class="reason"
					>{total - creates} existing {total - creates === 1 ? 'file is' : 'files are'} replaced; check
					what changes first.</span
				>{/if}
		</div>
	{:else}
		<div class="head">
			<h3 id={headId} class="insp-title">
				{#if error}<Icon name="cross" size="glyph" tone="fail" />{error}{:else}Paste the {packet
						? 'packet'
						: 'reply'} back{/if}
			</h3>
			<button class="btn" type="button" onclick={trigger}
				><Icon name="paste" />Paste back<kbd>⌘⇧V</kbd></button
			>
		</div>
		{#if fixable}
			<div class="actions">
				<button class="btn" type="button" onclick={copyFix}
					><Icon name="copy" />Copy fix request</button
				>
				<span class="reason">The packet was written with problems. Ask Claude to fix them.</span>
			</div>
		{/if}
		<p class="hint">
			{packet
				? 'Copy the packet Claude prints after you say "approved". Foreman validates it before writing anything.'
				: 'Copy Claude’s reply. Foreman shows the exact writes before anything is written.'}
		</p>
	{/if}
</div>

<style>
	.head {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}
	.head kbd,
	.head .btn {
		margin-left: auto;
	}
	.insp-title {
		display: inline-flex;
		gap: var(--space-3);
		align-items: center;
	}
</style>
