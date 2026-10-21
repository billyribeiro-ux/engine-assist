<script lang="ts">
	// Today (F-017, mockup today.html): what waits on you (P2 inbox), then each project's next step
	// with its spec line and primary action. The top row has focus (AC-F017-01); ⌘⇧↵ runs it from
	// anywhere (AC-F017-02); a project with nothing runnable says why (AC-F017-04, -05).
	import { goto } from '$app/navigation';
	import { onMount, tick } from 'svelte';

	import { report, runPrepared, runStep } from '#lib/actions/run.ts';
	import { failureText } from '#lib/actions/words.ts';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import Icon from '#lib/components/Icon.svelte';
	import MiniStrip from '#lib/components/MiniStrip.svelte';
	import SpecLine from '#lib/components/SpecLine.svelte';
	import type { IconName } from '#lib/components/icons.ts';
	import type { InboxItem, NextStepRow, RunSummary } from '#lib/ipc/types.ts';
	import { registerShortcuts } from '#lib/keyboard/registry.ts';
	import { markColdStart } from '#lib/perf.ts';
	import { sortInbox } from '#lib/nav/inbox.ts';
	import { to } from '#lib/nav/routes.ts';
	import type { Tone } from '#lib/nav/status.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { terminal } from '#lib/stores/terminal.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';
	import { SHEET } from '#lib/nav/status.ts';

	ui.setChrome({ title: 'Today' });

	const today = resource(
		() => null,
		(c) => c.todayGet({}),
		() => ['today']
	);

	// Sessions that were running when Foreman last quit or crashed (SR-57, AC-F020-10): listed with
	// Resume, never relaunched by themselves.
	const boot = resource(
		() => null,
		(c) => c.appBootstrap({}),
		() => ['runs', 'projects']
	);
	const interrupted: RunSummary[] = $derived(boot.value?.interrupted ?? []);
	async function resume(r: RunSummary): Promise<void> {
		const c = await ipc();
		try {
			const prep = await c.chainResumePrepare({ chain: r.chain });
			report(
				await runPrepared(
					{ projectId: r.project, step: r.step ?? '', surface: 'cli', entry: 'stageView' },
					prep
				)
			);
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}

	const rows: NextStepRow[] = $derived(today.value?.rows ?? []);
	const inbox: InboxItem[] = $derived(sortInbox(today.value?.inbox ?? []));
	const projectCount = $derived(new Set(inbox.map((i) => i.project)).size);

	const INBOX_LOOK: Record<InboxItem['kind'], { icon: IconName; tone: Tone; action: string }> = {
		sessionBlocked: { icon: 'warn', tone: 'attention', action: 'Open terminal' },
		sessionYourTurn: { icon: 'terminal', tone: 'accent', action: 'Open terminal' },
		packetLanded: { icon: 'file', tone: 'attention', action: 'Open packet' },
		packetInvalid: { icon: 'cross', tone: 'fail', action: 'Open packet' },
		gateFailed: { icon: 'cross', tone: 'fail', action: 'Open gate' },
		gateSignable: { icon: 'stamp', tone: 'attention', action: 'Review and sign' },
		outsideChange: { icon: 'warn', tone: 'attention', action: 'Review change' },
		tamper: { icon: 'cross', tone: 'fail', action: 'Review packet' },
		chatAwaitingPaste: { icon: 'paste', tone: 'attention', action: 'Open step' },
		waitElapsed: { icon: 'clock', tone: 'attention', action: 'Open step' },
		interruptedSession: {
			icon: 'broken',
			tone: 'muted',
			action: 'Start a fresh chain with context'
		},
		stepReady: { icon: 'ring', tone: 'muted', action: 'Open step' }
	};

	async function act(i: InboxItem): Promise<void> {
		switch (i.kind) {
			case 'sessionBlocked':
			case 'sessionYourTurn':
				if (i.run) terminal.active = i.run;
				await goto(i.stage !== null ? to.stage(i.project, i.stage) : to.project(i.project));
				if (!ui.drawerOpen) ui.toggleDrawer();
				return;
			case 'packetLanded':
			case 'packetInvalid':
			case 'tamper':
				return goto(to.packet(i.project, i.stage ?? 0));
			case 'gateFailed':
			case 'gateSignable':
				return goto(to.gate(i.project, i.stage ?? 0));
			case 'outsideChange':
				return goto(to.changes(i.project));
			case 'interruptedSession':
				if (i.step)
					report(
						await runStep({
							projectId: i.project,
							step: i.step,
							surface: 'cli',
							entry: 'doNext',
							mode: 'freshWithContext'
						})
					);
				return;
			default:
				return goto(
					i.step && i.stage !== null ? to.step(i.project, i.stage, i.step) : to.project(i.project)
				);
		}
	}

	/** The project's current stage has its packet waiting for Billy's signature. */
	function signable(row: NextStepRow): boolean {
		return row.strip[row.project.currentStage ?? 0] === 'waiting';
	}

	function primary(row: NextStepRow): { label: string; icon: IconName; disabled: string | null } {
		const s = row.next;
		if (!s && !signable(row)) return { label: 'Open stage', icon: 'open', disabled: null };
		if (!s) return { label: 'Review and sign', icon: 'pen', disabled: null };
		const why = s.whyNotRunnable;
		const disabled = why
			? why.startsWith('Waits for')
				? `${why.split(':')[0] ?? why} to finish`
				: why
			: null;
		if (s.surface === 'chat') return { label: 'Copy and open', icon: 'copy', disabled };
		return { label: 'Run in terminal', icon: 'play', disabled };
	}

	async function runRow(row: NextStepRow): Promise<void> {
		const s = row.next;
		if (!s) {
			if (row.project.readOnly) return;
			await goto(
				signable(row)
					? to.gate(row.project.id, row.project.currentStage ?? 0)
					: to.stage(row.project.id, row.project.currentStage ?? 0)
			);
			return;
		}
		const p = primary(row);
		if (p.disabled) return ui.say(p.disabled);
		report(
			await runStep({
				projectId: row.project.id,
				step: s.key,
				surface: s.surface,
				entry: 'stageView'
			})
		);
	}

	function stripLabel(row: NextStepRow): string {
		const cur = row.project.currentStage ?? 0;
		const signed = row.strip.map((s, i) => (s === 'signed' ? i : -1)).filter((i) => i > 0);
		const st = row.strip[cur];
		const state =
			st === 'waiting' ? 'packet waiting for your signature' : st ? SHEET[st].word : 'in progress';
		const sig = signed.length
			? `; ${signed.length === 1 ? `stage ${signed[0] ?? ''}` : `stages ${signed[0] ?? ''} to ${signed.at(-1) ?? ''}`} signed`
			: '';
		return `Stage ${cur} of 19 ${state === 'in progress' ? 'in progress' : `, ${state}`}${sig}`.replace(
			' ,',
			','
		);
	}

	let list = $state<HTMLElement>();
	function rowsEls(): HTMLElement[] {
		return list ? [...list.querySelectorAll<HTMLElement>('[data-today-row]')] : [];
	}
	function move(d: number): true {
		const els = rowsEls();
		const i = els.findIndex(
			(el) => el === document.activeElement || el.contains(document.activeElement)
		);
		const next = els[Math.max(0, Math.min(els.length - 1, (i < 0 ? -1 : i) + d))];
		next?.focus();
		return true;
	}

	onMount(() =>
		registerShortcuts({
			nextStep: () => move(1),
			prevStep: () => move(-1),
			run: async () => {
				const top = rows[0];
				if (!top) return 'No project has a next step yet.';
				await runRow(top);
				return true;
			}
		})
	);

	let focused = false;
	$effect(() => {
		if (focused || !today.value) return;
		focused = true;
		void tick().then(() => {
			document
				.querySelector<HTMLElement>('.next [data-today-row], .empty-start [data-today-row]')
				?.focus();
			markColdStart();
		});
	});
</script>

<main class="main" id="main" aria-labelledby="today-title">
	<div class="page" bind:this={list}>
		<h1 class="page-title" id="today-title">Today</h1>
		{#if today.error}
			<p class="err" role="alert"><Icon name="cross" size="glyph" />{today.error.message}</p>
		{:else if today.value && rows.length === 0}
			<div class="empty-start">
				<h2 class="hero">Start your first project</h2>
				<p class="page-sub">
					A project is one product in its own folder; Foreman tailors the blueprint to it and shows
					the next step here.
				</p>
				<div class="actions">
					<a class="btn primary lg" href={to.setup()} data-today-row
						>Start your first project<kbd>⌘N</kbd></a
					>
				</div>
			</div>
		{:else if today.value}
			<p class="page-sub">
				{#if inbox.length}{inbox.length} things are waiting on you across {projectCount}
					{projectCount === 1 ? 'project' : 'projects'}.{/if} Press ⌘⇧↵ from anywhere to run the top next
				step.
			</p>

			{#if inbox.length}
				<section class="section" aria-labelledby="inbox-h">
					<div class="section-head">
						<h2 class="section-title" id="inbox-h">Waiting on you</h2>
						<span class="muted">J and K move, Return does the action</span>
					</div>
					<ol class="inbox" aria-label="Waiting on you, in priority order">
						{#each inbox as i, n (n)}
							{@const look = INBOX_LOOK[i.kind]}
							<li>
								<Icon name={look.icon} size="glyph" tone={look.tone} />
								<span class="proj">{i.projectName}</span>
								<span class="what">{i.title}<small>{i.detail}</small></span>
								<button class="btn" type="button" data-today-row onclick={() => act(i)}
									>{#if i.kind === 'sessionBlocked'}<Icon
											name="terminal"
										/>{/if}{look.action}</button
								>
							</li>
						{/each}
					</ol>
				</section>
			{/if}

			{#if interrupted.length}
				<section class="section" aria-labelledby="int-h">
					<div class="section-head">
						<h2 class="section-title" id="int-h">Interrupted sessions</h2>
						<span class="muted"
							>Foreman quit while these were running. Nothing restarts by itself.</span
						>
					</div>
					<ol class="inbox" aria-label="Interrupted sessions">
						{#each interrupted as r (r.runId)}
							<li>
								<Icon name="broken" size="glyph" tone="muted" />
								<span class="proj">{r.projectName}</span>
								<span class="what"
									>{r.step ?? ''}
									{r.stepTitle}<small
										><code>{r.tabName}</code>{#if !r.resumable && r.whyNotResumable}, {r.whyNotResumable}{/if}</small
									></span
								>
								{#if r.resumable}
									<button class="btn" type="button" data-today-row onclick={() => resume(r)}
										><Icon name="play" />Resume</button
									>
								{:else}
									<span class="muted small">Can't resume</span>
								{/if}
							</li>
						{/each}
					</ol>
				</section>
			{/if}

			<section class="section" aria-labelledby="next-h">
				<div class="section-head">
					<h2 class="section-title" id="next-h">Next step in each project</h2>
				</div>
				<ol class="next">
					{#each rows as row, n (row.project.id)}
						{@const p = primary(row)}
						<li class={[n === 0 && 'is-top']}>
							<div class="proj">
								<b>{row.project.name}</b><MiniStrip statuses={row.strip} label={stripLabel(row)} />
							</div>
							<div>
								{#if row.next}
									<div class="stepname">
										{row.next.number}
										{row.next.title}
										<span class="muted"
											>{row.next.specLine?.where.includes('continues')
												? 'continues the council session'
												: `Stage ${row.project.currentStage ?? ''}, ${row.stageTitle ?? ''}`}</span
										>
									</div>
									{#if row.next.specLine}<SpecLine
											line={row.next.specLine}
											id="today-{row.project.id}"
											compact
										/>{/if}
								{:else if row.whyNone && row.project.readOnly}
									<div class="stepname">{row.whyNone}</div>
									<p class="small muted">Nothing here runs until the folder is back.</p>
								{:else if signable(row)}
									<div class="stepname">
										Sign the Stage {row.project.currentStage} packet
										<span class="muted">{row.stageTitle} close-out</span>
									</div>
									<p class="small muted">{row.whyNone}</p>
								{:else}
									<div class="stepname">
										{row.whyNone ?? 'Nothing to run yet.'}
										<span class="muted"
											>Stage {row.project.currentStage ?? ''}, {row.stageTitle ?? ''}</span
										>
									</div>
								{/if}
							</div>
							<div class="right">
								{#if row.project.readOnly}
									<span class="reason">Nothing to run</span>
								{:else}
									<button
										class={['btn', row.next && 'primary']}
										type="button"
										data-today-row
										aria-disabled={p.disabled ? 'true' : undefined}
										aria-describedby={p.disabled ? `why-${row.project.id}` : undefined}
										onclick={() => runRow(row)}
										><Icon name={p.icon} />{p.label}{#if n === 0 && !p.disabled}<kbd>⌘↵</kbd
											>{/if}</button
									>
									{#if p.disabled}
										<span class="reason" id="why-{row.project.id}">{p.disabled}</span>
									{:else if n === 0}
										<kbd>Do next ⌘⇧↵</kbd>
									{/if}
								{/if}
							</div>
						</li>
					{/each}
				</ol>
				<p class="hintline"><span>New project ⌘N</span><span>Commands ⌘K</span></p>
			</section>
		{/if}
	</div>
</main>

<style>
	.inbox li {
		display: grid;
		grid-template-columns: 20px 112px minmax(0, 1fr) auto;
		gap: var(--space-4);
		align-items: center;
		padding: 10px var(--space-3) 10px var(--space-2);
		border-bottom: var(--hairline) solid var(--rule);
	}
	.inbox li:first-child,
	.inbox li:focus-within {
		background: var(--sheet-raised);
	}
	.inbox .proj {
		font-weight: var(--weight-medium);
	}
	.inbox .what {
		min-width: 0;
	}
	.inbox .what small {
		display: block;
		color: var(--ink-muted);
		font-size: var(--text-12);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.next li {
		display: grid;
		grid-template-columns: 196px minmax(0, 1fr) auto;
		gap: var(--space-5);
		align-items: center;
		padding: 14px var(--space-3) 14px var(--space-2);
		border-bottom: var(--hairline) solid var(--rule);
	}
	.next li.is-top {
		background: var(--sheet-raised);
		box-shadow: inset 2px 0 0 var(--accent-ink);
	}
	.next .proj {
		display: grid;
		gap: 6px;
	}
	.next .proj b {
		font-size: var(--text-15);
		font-weight: var(--weight-semibold);
	}
	.stepname {
		font-weight: var(--weight-medium);
	}
	.stepname .muted {
		font-weight: var(--weight-regular);
	}
	.next .right {
		display: grid;
		justify-items: end;
		gap: var(--space-2);
	}
	.small {
		font-size: var(--text-12);
	}
	.hintline {
		display: flex;
		gap: var(--space-5);
		margin-top: var(--space-3);
		font-size: var(--text-12);
		color: var(--ink-muted);
	}
	.empty-start {
		display: grid;
		gap: var(--space-3);
		margin-top: var(--space-7);
	}
	.hero {
		font-size: var(--text-28);
		font-weight: var(--weight-semibold);
		letter-spacing: -0.015em;
		line-height: 1.15;
	}
</style>
