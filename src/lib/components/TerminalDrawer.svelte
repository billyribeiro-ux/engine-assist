<script lang="ts">
	// Terminal drawer (components.md "Terminal tab and drawer", F-009): grip, tab bar, Now/Next strip
	// and the active tab's xterm. ⌃` toggles it and its height is remembered (AC-F030-05).
	import { failureText } from '#lib/actions/words.ts';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import { RUN } from '#lib/nav/status.ts';
	import { refresh } from '#lib/stores/bus.ts';
	import { current } from '#lib/stores/project.svelte.ts';
	import { terminal } from '#lib/stores/terminal.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import Icon from './Icon.svelte';
	import NowNext from './NowNext.svelte';
	import TerminalView from './TerminalView.svelte';

	let {
		projectName = null,
		fontSize = 12,
		scrollback = 10000,
		macOptionIsMeta = true,
		screenReaderMode = false
	}: {
		projectName?: string | null;
		fontSize?: number;
		scrollback?: number;
		macOptionIsMeta?: boolean;
		screenReaderMode?: boolean;
	} = $props();

	$effect(() => {
		terminal.scrollback = scrollback;
	});

	const tabs = $derived(terminal.tabs);
	const active = $derived(tabs.find((t) => t.runId === terminal.active) ?? tabs[0] ?? null);
	let menuOpen = $state(false);
	let drag: { y: number; h: number } | null = null;

	function lineCount(n: number): string {
		return `${n.toLocaleString('en-US')} ${n === 1 ? 'line' : 'lines'}`;
	}

	function word(t: (typeof tabs)[number]): string {
		if (t.status === 'exited') return t.exitCode === null ? 'exited' : `exited ${t.exitCode}`;
		return RUN[t.status].word;
	}

	async function act(kind: 'interrupt' | 'terminate' | 'copy' | 'resume'): Promise<void> {
		menuOpen = false;
		if (!active) return;
		const client = await ipc();
		try {
			if (kind === 'interrupt') await client.ptyInterrupt({ runId: active.runId });
			if (kind === 'terminate') {
				await client.ptyTerminate({ runId: active.runId });
				terminal.setStatus(active.runId, 'exited', 143);
				refresh(
					'runs',
					'today',
					...(active.projectId ? ([`project:${active.projectId}`] as const) : [])
				);
			}
			if (kind === 'copy') {
				if (!active.projectId) return;
				await client.clipboardCopy({
					projectId: active.projectId,
					target: 'sessionId',
					step: null,
					chain: active.chain,
					path: null,
					stage: null
				});
				ui.toast('Session ID copied', 'pass', 'copy');
			}
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			ui.say(f.code === 'cancelled' ? 'Cancelled. The session keeps running.' : failureText(f));
		}
	}

	async function close(runId: string, status: string): Promise<void> {
		if (
			status === 'running' ||
			status === 'starting' ||
			status === 'idle' ||
			status === 'blocked'
		) {
			// Closing a running tab ends the session; Rust asks first in a native dialog.
			const client = await ipc();
			try {
				await client.ptyTerminate({ runId });
			} catch (e: unknown) {
				const f = toIpcFailure(e);
				ui.say(f.code === 'cancelled' ? 'Cancelled. The session keeps running.' : failureText(f));
				return;
			}
		}
		terminal.close(runId);
	}

	function onGripKey(e: KeyboardEvent): void {
		if (e.key === 'ArrowUp') ui.setDrawerHeight(ui.drawerHeight + 16);
		else if (e.key === 'ArrowDown') ui.setDrawerHeight(ui.drawerHeight - 16);
		else return;
		e.preventDefault();
	}
	function onGripDown(e: PointerEvent): void {
		drag = { y: e.clientY, h: ui.drawerHeight };
		(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
	}
	function onGripMove(e: PointerEvent): void {
		if (drag) ui.setDrawerHeight(drag.h + (drag.y - e.clientY));
	}

	function onTabKey(e: KeyboardEvent, i: number): void {
		const n = tabs.length;
		const next =
			e.key === 'ArrowRight' ? (i + 1) % n : e.key === 'ArrowLeft' ? (i - 1 + n) % n : -1;
		if (next < 0) return;
		e.preventDefault();
		const t = tabs[next];
		if (t) {
			terminal.active = t.runId;
			queueMicrotask(() => document.getElementById(`tab-${t.runId}`)?.focus());
		}
	}

	const heightStyle = (el: HTMLElement) => {
		$effect(() => {
			el.style.setProperty('--drawer-h', `${ui.drawerHeight}px`);
		});
	};
</script>

{#if tabs.length === 0 || !ui.drawerOpen}
	<section class="drawer collapsed" aria-label="Terminal drawer">
		<div class="drawer-bar">
			{#if tabs.length === 0}
				<span class="muted note"
					>Terminal: no sessions{projectName ? ` in ${projectName}` : ''}. Chat steps run in
					claude.ai.</span
				>
			{:else}
				<span class="muted note"
					>Terminal: {tabs.length} {tabs.length === 1 ? 'session' : 'sessions'}, hidden.</span
				>
				<button class="btn ghost" type="button" onclick={() => ui.toggleDrawer()}>Show</button>
			{/if}
			<span class="spacer"></span><kbd title="Toggle terminal drawer">⌃`</kbd>
		</div>
	</section>
{:else}
	<section
		class={['drawer', terminal.focusMode && 'focus-mode']}
		aria-label="Terminal drawer"
		{@attach heightStyle}
	>
		<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (a focusable separator is the ARIA splitter pattern: ↑ ↓ resize) -->
		<div
			class="drawer-grip"
			role="separator"
			aria-orientation="horizontal"
			aria-label="Resize terminal drawer"
			tabindex="0"
			aria-valuenow={ui.drawerHeight}
			aria-valuemin={120}
			aria-valuemax={640}
			onkeydown={onGripKey}
			onpointerdown={onGripDown}
			onpointermove={onGripMove}
			onpointerup={() => (drag = null)}
		></div>
		<div class="drawer-bar">
			<div class="tablist" role="tablist" aria-label="Terminal sessions">
				{#each tabs as t, i (t.runId)}
					{@const look = RUN[t.status]}
					<button
						class="term-tab"
						role="tab"
						id="tab-{t.runId}"
						aria-selected={t.runId === active?.runId}
						aria-controls="term-panel"
						tabindex={t.runId === active?.runId ? 0 : -1}
						onclick={() => (terminal.active = t.runId)}
						onkeydown={(e) => onTabKey(e, i)}
						><Icon
							name={t.status === 'exited' && (t.exitCode ?? 0) !== 0 ? 'cross' : look.icon}
							size="glyph"
							tone={t.status === 'exited' && (t.exitCode ?? 0) !== 0 ? 'fail' : look.tone}
						/>{t.tabName}<span class="state">{word(t)}</span></button
					>
				{/each}
			</div>
			<span class="spacer"></span>
			<button
				class="btn ghost icon-only"
				type="button"
				aria-label="Focus mode (⌘⇧T)"
				aria-pressed={terminal.focusMode}
				title="Focus mode ⌘⇧T"
				onclick={() => (terminal.focusMode = !terminal.focusMode)}><Icon name="open" /></button
			>
			{#if active && (active.status === 'running' || active.status === 'idle' || active.status === 'blocked')}
				<button class="btn ghost" type="button" onclick={() => act('interrupt')}>Interrupt</button>
			{/if}
			<div class="menu-wrap">
				<button
					class="btn ghost icon-only"
					type="button"
					aria-label="Session menu"
					aria-haspopup="menu"
					aria-expanded={menuOpen}
					onclick={() => (menuOpen = !menuOpen)}><Icon name="chev-down" /></button
				>
				{#if menuOpen && active}
					<div class="menu" role="menu" aria-label="Session">
						<button role="menuitem" type="button" onclick={() => act('interrupt')}
							>Interrupt (Ctrl-C)</button
						>
						<button role="menuitem" type="button" onclick={() => act('terminate')}
							>Terminate…</button
						>
						<button role="menuitem" type="button" onclick={() => act('copy')}
							>Copy session ID</button
						>
						<button role="menuitem" type="button" onclick={() => close(active.runId, active.status)}
							>Close tab</button
						>
					</div>
				{/if}
			</div>
			<kbd title="Toggle terminal drawer">⌃`</kbd>
		</div>
		{#if current.project && active?.projectId === current.project.summary.id}
			<NowNext project={current.project} />
		{/if}
		{#if active}
			<div class="term" id="term-panel" role="tabpanel" aria-labelledby="tab-{active.runId}">
				{#if active.archived}
					<details class="archived">
						<summary
							><Icon name="chev" class="chev" />Earlier in this session: {active.archived.label}, {lineCount(
								active.archived.lines
							)} (archived, read-only)</summary
						>
						<!-- svelte-ignore a11y_no_noninteractive_tabindex (a scrollable region must be keyboard-reachable) -->
						<pre
							class="archive-text"
							tabindex="0"
							aria-label="Archived output, {active.archived.label}">{active.archived.text}</pre>
					</details>
				{/if}
				{#key active.runId}
					<TerminalView
						runId={active.runId}
						live={active.status === 'running' ||
							active.status === 'idle' ||
							active.status === 'blocked' ||
							active.status === 'starting'}
						label="Terminal output, {active.tabName}"
						{fontSize}
						{scrollback}
						{macOptionIsMeta}
						{screenReaderMode}
					/>
				{/key}
			</div>
		{/if}
	</section>
{/if}

<style>
	.drawer {
		flex: none;
		border-top: var(--hairline) solid var(--rule);
		background: var(--term-bg);
		color: var(--term-ink);
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		grid-template-rows: auto auto auto minmax(0, 1fr);
		min-width: 0;
		overflow: hidden;
		height: min(var(--drawer-h, var(--drawer-height)), 60vh);
		min-height: 0;
		transition: height var(--dur-drawer) var(--ease-out);
	}
	.drawer.focus-mode {
		height: 70vh;
	}
	.drawer.collapsed {
		height: auto;
		grid-template-rows: auto;
	}
	.drawer-grip {
		height: 4px;
		margin-top: -2px;
		cursor: row-resize;
	}
	.drawer-bar {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: 0 var(--space-4) 0 var(--space-3);
		height: 32px;
		border-bottom: var(--hairline) solid var(--term-rule);
		background: var(--ground);
	}
	.drawer.collapsed .drawer-bar {
		border-bottom: 0;
	}
	.drawer-bar .spacer {
		flex: 1;
	}
	.note {
		font-size: var(--text-12);
	}
	.tablist {
		display: flex;
		gap: var(--space-2);
		min-width: 0;
		overflow-x: auto;
	}
	.term-tab {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		height: 24px;
		padding: 0 10px;
		border-radius: var(--radius-control);
		border: 0;
		background: transparent;
		font-family: var(--font-mono);
		font-size: var(--text-12);
		color: var(--ink-muted);
		white-space: nowrap;
	}
	.term-tab[aria-selected='true'] {
		background: var(--term-bg);
		color: var(--term-ink);
		box-shadow: inset 0 0 0 1px var(--term-rule);
	}
	.term-tab .state {
		font-family: var(--font-ui);
		font-size: var(--text-11);
		color: var(--ink-muted);
	}
	.term-tab[aria-selected='true'] .state {
		color: var(--term-muted);
	}
	.term {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		grid-template-rows: auto minmax(0, 1fr);
		min-height: 0;
		padding-top: var(--space-2);
	}
	.archived summary {
		font-family: var(--font-ui);
		color: var(--term-muted);
		font-size: var(--text-12);
		list-style: none;
		display: inline-flex;
		gap: 6px;
		align-items: center;
		padding: 0 var(--space-5);
	}
	.archived summary::-webkit-details-marker {
		display: none;
	}
	.archive-text {
		margin: var(--space-2) var(--space-5) 0;
		max-height: 40vh;
		overflow: auto;
		font-family: var(--font-mono);
		font-size: var(--text-12);
		color: var(--term-muted);
		white-space: pre;
	}
	.menu-wrap {
		position: relative;
	}
	.menu {
		position: absolute;
		right: 0;
		bottom: 30px;
		z-index: 25;
		display: grid;
		min-width: 180px;
		padding: var(--space-2);
		background: var(--sheet-raised);
		color: var(--ink);
		border-radius: var(--radius-control);
		box-shadow: var(--shadow-overlay);
	}
	.menu button {
		text-align: left;
		border: 0;
		background: transparent;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-control);
		font-size: var(--text-13);
	}
	.menu button:hover,
	.menu button:focus-visible {
		background: var(--selected);
	}
</style>
