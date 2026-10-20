<script lang="ts">
	// Outside-change review (F-022, mockup outside-change.html): what changed and when, git state, the
	// approved hash, then one redacted diff per decision file with its effect and consequences.
	// Reject restores the approved content (atomic write); Accept only *requests* acceptance: Rust asks
	// "Accept this change to <file>? It changes: <keys>" in a native dialog (SR-24a, AC-F022-02). If
	// the file changes again meanwhile, the action is refused and the new diff shown (AC-F022-05).
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';

	import { failureText } from '#lib/actions/words.ts';
	import Icon from '#lib/components/Icon.svelte';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { PendingChange, ProjectView } from '#lib/ipc/types.ts';
	import { registerShortcuts } from '#lib/keyboard/registry.ts';
	import { to } from '#lib/nav/routes.ts';
	import Diff from '#lib/render/Diff.svelte';
	import Text from '#lib/render/Text.svelte';
	import { refresh } from '#lib/stores/bus.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	let { project }: { project: ProjectView } = $props();
	const pid = $derived(project.summary.id);
	$effect(() => {
		ui.setChrome({ title: project.summary.name, sub: 'Pending outside change' });
	});

	const pending = resource(
		() => pid,
		(c, id) => c.pendingChangesList({ projectId: id }),
		(id) => [`pending:${id}`, `project:${id}`]
	);
	const changes: PendingChange[] = $derived(pending.value ?? []);
	let notice = $state<string | null>(null);
	let busy = $state(false);

	const FILE: Record<PendingChange['file'], string> = {
		project: '.foreman/project.json',
		overlay: '.foreman/overlay.json',
		lock: '.foreman/blueprint.lock.json'
	};
	const EFFECT: Record<PendingChange['effect'], { text: string; tone: 'attention' | 'pass' }> = {
		changesPath: { text: 'Changes the path', tone: 'attention' },
		noEffect: { text: 'Has no effect', tone: 'attention' },
		reloaded: { text: 'Reloaded', tone: 'pass' }
	};

	async function decide(accept: boolean): Promise<true | string> {
		if (changes.length === 0) return 'Nothing is pending.';
		const c = await ipc();
		busy = true;
		notice = null;
		try {
			for (const ch of changes) {
				const req = { projectId: pid, change: ch.id, seenSha256: ch.seenSha256 };
				if (accept) await c.pendingChangeAccept(req);
				else await c.pendingChangeReject(req);
			}
			ui.toast(
				accept
					? 'Changes accepted. They are the new approved versions.'
					: 'Approved versions restored.'
			);
			refresh(`project:${pid}`, `pending:${pid}`, 'today');
			await goto(to.stage(pid, project.summary.currentStage ?? 1));
			return true;
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			if (f.code === 'stale') {
				notice =
					'The file changed again while you were reviewing. Here is the new diff; nothing was accepted or restored.';
				await pending.reload();
				return true;
			}
			if (f.code === 'cancelled') return 'Cancelled. The change stays pending.';
			return failureText(f);
		} finally {
			busy = false;
		}
	}

	onMount(() =>
		registerShortcuts({
			run: () => decide(true)
		})
	);
	const ctx = $derived(changes[0]?.context ?? null);
	const variablesOnly = $derived(
		changes
			.flatMap((c) => c.consequences)
			.find((x) => x.glyph === 'check' && x.text.includes('variable')) ?? null
	);
	let toasted = false;
	$effect(() => {
		if (variablesOnly && !toasted) {
			toasted = true;
			ui.toast('Variables reloaded: QUALITY_BAR changed in project.json.');
		}
	});
</script>

<div class="body no-inspector">
	<main class="main oc" id="main" aria-labelledby="oc-title">
		<div class="page wide">
			<h1 class="page-title" id="oc-title">
				Review a change to {project.summary.name}'s decision files
			</h1>
			{#if changes.length}
				<p class="page-sub">
					Something other than Foreman edited {changes.length === 1
						? 'a file'
						: `${changes.length} files`} that decide how this project runs. Foreman keeps using the approved
					versions until you accept.
				</p>
				{#if notice}<p class="banner-inline" role="alert">
						<Icon name="warn" size="glyph" tone="attention" />{notice}
					</p>{/if}
				{#if ctx}
					<dl class="oc-meta">
						<dt>Changed</dt>
						<dd><Text text={ctx.changed} /></dd>
						<dt>Git</dt>
						<dd>{ctx.git}</dd>
						<dt>Approved version</dt>
						<dd>{ctx.approved}</dd>
					</dl>
				{/if}
				{#each changes as ch (ch.id)}
					<div class="filehead">
						<h2>{FILE[ch.file]}</h2>
						<span class="pill {EFFECT[ch.effect].tone}"
							><Icon name="warn" size="glyph" tone={EFFECT[ch.effect].tone} />{EFFECT[ch.effect]
								.text}</span
						>
					</div>
					<Diff redacted={ch.diff} label="Diff of {FILE[ch.file].split('/').at(-1)}" />
					<ul class="effect">
						{#each ch.consequences as q, i (i)}
							<li>
								<Icon
									name={q.glyph}
									size="glyph"
									tone={q.glyph === 'check' ? 'pass' : 'muted'}
								/><span>{q.text}</span>
							</li>
						{/each}
					</ul>
				{/each}
			{:else if pending.value}
				<div class="empty">
					<strong>Nothing is pending</strong>
					<span>The decision files match their approved versions.</span>
					<a class="btn" href={to.stage(pid, project.summary.currentStage ?? 1)}
						>Back to the stage</a
					>
				</div>
			{/if}
		</div>
		{#if changes.length}
			<div class="decide">
				<span class="reason"
					>Reject restores the approved versions with an atomic write. Accept asks you once more in
					a macOS dialog, then makes them the new approved versions.</span
				>
				<span class="spacer"></span>
				<button
					class="btn"
					type="button"
					disabled={busy}
					onclick={() => decide(false).then((r) => r !== true && ui.say(r))}
					><Icon name="restore" />Reject and restore</button
				>
				<button
					class="btn primary"
					type="button"
					disabled={busy}
					onclick={() => decide(true).then((r) => r !== true && ui.say(r))}
					><Icon name="check" />Accept changes<kbd>⌘↵</kbd></button
				>
			</div>
		{/if}
	</main>
</div>

<style>
	.oc {
		display: grid;
		grid-template-rows: minmax(0, 1fr) auto;
		overflow: hidden;
	}
	.oc > .page {
		overflow: auto;
	}
	.wide {
		max-width: none;
	}
	.oc-meta {
		display: grid;
		grid-template-columns: 140px minmax(0, 1fr);
		gap: 6px var(--space-4);
		font-size: var(--text-12);
		margin-top: var(--space-5);
		max-width: 760px;
	}
	.oc-meta dt {
		color: var(--ink-muted);
	}
	.filehead {
		display: flex;
		align-items: baseline;
		gap: var(--space-4);
		margin: var(--space-6) 0 var(--space-3);
	}
	.filehead h2 {
		font-size: var(--text-13);
		font-weight: var(--weight-semibold);
		font-family: var(--font-mono);
	}
	.effect {
		margin-top: var(--space-3);
		display: grid;
		gap: 6px;
		font-size: var(--text-12);
		max-width: 760px;
	}
	.effect li {
		display: flex;
		gap: var(--space-3);
		align-items: flex-start;
	}
	.decide {
		display: flex;
		gap: var(--space-3);
		align-items: center;
		padding: var(--space-4) var(--space-7);
		border-top: var(--hairline) solid var(--rule);
		background: var(--sheet);
	}
	.decide .spacer {
		flex: 1;
	}
	.oc :global(.diff) {
		max-width: 860px;
	}
	.banner-inline {
		display: flex;
		gap: var(--space-3);
		align-items: center;
		margin-top: var(--space-4);
		padding: var(--space-3) var(--space-4);
		background: var(--attention-wash);
		border-left: 3px solid var(--attention);
		font-size: var(--text-12);
	}
</style>
