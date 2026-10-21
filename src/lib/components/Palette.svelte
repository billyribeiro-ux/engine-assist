<script lang="ts">
	// Command palette (components.md, F-028): ⌘K from anywhere, including the terminal. A combobox
	// over a grouped listbox; ↑↓ move, ↵ opens, ⌘↵ runs a step, Esc closes and returns focus. The
	// rest of the window is inert while it is open.
	import { goto } from '$app/navigation';
	import { tick } from 'svelte';

	import { openTerminalHere, removeProject, togglePin } from '#lib/actions/project.ts';
	import { doNext, report, runStep } from '#lib/actions/run.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { ProjectView, Surface } from '#lib/ipc/types.ts';
	import { type ActionId, handlerFor, NOT_HERE, SHORTCUTS } from '#lib/keyboard/registry.ts';
	import { to } from '#lib/nav/routes.ts';
	import { current } from '#lib/stores/project.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import Icon from './Icon.svelte';
	import type { IconName } from './icons.ts';

	interface Item {
		id: string;
		group: 'Steps' | 'Actions' | 'Projects' | 'Stages';
		icon: IconName;
		title: string;
		sub: string;
		hint: string;
		/** Extra words the search matches (the selected step's title for its actions). */
		keywords?: string;
		/** A step's project and stage (N6: the Steps group filters by ID, not by name). */
		projectId?: string;
		stage?: number;
		open: () => unknown;
		run?: () => unknown;
	}

	let query = $state('');
	let views = $state<ProjectView[]>([]);
	let active = $state(0);
	let input = $state<HTMLInputElement>();
	let returnTo: Element | null = null;

	$effect(() => {
		if (!ui.paletteOpen) return;
		returnTo = document.activeElement;
		query = '';
		active = 0;
		void (async () => {
			const c = await ipc();
			const list = await c.projectsList({});
			views = await Promise.all(list.map((p) => c.projectOpen({ projectId: p.id })));
			await tick();
			input?.focus();
		})();
		const app = document.querySelector('.app');
		app?.setAttribute('inert', '');
		return () => {
			app?.removeAttribute('inert');
		};
	});

	function close(): void {
		ui.paletteOpen = false;
		if (returnTo instanceof HTMLElement) returnTo.focus();
	}

	const SURFACE_ICON: Record<Surface, IconName> = {
		cli: 'terminal',
		chat: 'chat',
		desktop: 'desktop'
	};

	// The A.7 shortcuts that act on the selection or the screen (AC-F028-05). The palette runs the
	// same handler the shortcut does, so a refusal says the same one line (AC-F028-03).
	const SHORTCUT_ACTIONS: { id: ActionId; icon: IconName; keywords: string }[] = [
		{ id: 'run', icon: 'play', keywords: 'launch start' },
		{ id: 'copyCommand', icon: 'copy', keywords: 'argv claude' },
		{ id: 'nextStage', icon: 'chev', keywords: 'forward' },
		{ id: 'prevStage', icon: 'chev', keywords: 'back' },
		{ id: 'openGate', icon: 'check', keywords: 'inspection sign close out' },
		{ id: 'toggleInspector', icon: 'inspector', keywords: 'panel details' },
		{ id: 'pasteBack', icon: 'paste', keywords: 'chat reply output' },
		{ id: 'markDone', icon: 'check', keywords: 'finish chat step' },
		{ id: 'focusMode', icon: 'terminal', keywords: 'full screen' },
		{ id: 'returnFocus', icon: 'terminal', keywords: 'leave terminal workspace' }
	];

	async function viaShortcut(id: ActionId): Promise<void> {
		// The palette is closing; the window is inert until it has.
		await tick();
		const h = handlerFor(id);
		report(h ? await h() : NOT_HERE[id] || 'That action isn’t available here.');
	}

	const items: Item[] = $derived.by(() => {
		const out: Item[] = [];
		const sel = current.step;
		const proj = current.project;
		for (const v of views) {
			for (const s of v.stages)
				for (const st of s.steps) {
					if (st.state === 'needsAnnotation') continue;
					const state =
						st.state === 'done'
							? 'done'
							: st.next && st.whyNotRunnable
								? (st.whyNotRunnable.split(':')[0]?.toLowerCase() ?? '')
								: st.stateText.toLowerCase();
					out.push({
						id: `step-${v.summary.id}-${st.key}`,
						group: 'Steps',
						icon: st.surface ? SURFACE_ICON[st.surface] : 'play',
						title: `${st.number} ${st.title}`,
						sub: `${v.summary.name}, Stage ${s.stage}, ${state}`,
						hint: '↵ open',
						projectId: v.summary.id,
						stage: s.stage,
						open: () => goto(to.step(v.summary.id, s.stage, st.key)),
						run: () =>
							runStep({
								projectId: v.summary.id,
								step: st.key,
								surface: st.surface,
								entry: 'palette'
							}).then(report)
					});
				}
		}
		if (sel && proj) {
			out.push({
				id: 'copy-prompt',
				group: 'Actions',
				icon: 'copy',
				title: `Copy prompt for ${sel.number}`,
				sub: '',
				keywords: sel.title,
				hint: '⌘⇧C',
				open: async () => {
					const c = await ipc();
					await c.clipboardCopy({
						projectId: proj.summary.id,
						target: 'prompt',
						step: sel.key,
						chain: null,
						path: null,
						stage: null
					});
					ui.toast('Prompt copied', 'pass', 'copy');
				}
			});
		}
		if (proj) {
			const pr = proj.summary;
			out.push(
				{
					id: 'pin-project',
					group: 'Actions',
					icon: 'link',
					title: pr.pinned ? `Unpin ${pr.name}` : `Pin ${pr.name}`,
					sub: pr.pinned ? `from ⌘${pr.pinned}` : 'to a ⌘ number',
					hint: '',
					keywords: 'pin unpin shortcut slot',
					open: () => togglePin(pr.id)
				},
				{
					id: 'terminal-here',
					group: 'Actions',
					icon: 'terminal',
					title: 'Open terminal here',
					sub: `${pr.name}'s folder, in Terminal`,
					hint: '',
					keywords: 'terminal shell folder',
					open: () => openTerminalHere(pr.id)
				},
				{
					id: 'remove-project',
					group: 'Actions',
					icon: 'cross',
					title: `Remove ${pr.name}…`,
					sub: 'Foreman forgets it; the folder stays',
					hint: '',
					keywords: 'delete forget',
					open: () => removeProject(pr.id)
				}
			);
		}
		for (const a of SHORTCUT_ACTIONS) {
			const sc = SHORTCUTS.find((x) => x.id === a.id);
			if (!sc) continue;
			out.push({
				id: `act-${a.id}`,
				group: 'Actions',
				icon: a.icon,
				title: sc.label,
				sub: '',
				hint: sc.keys,
				keywords: a.keywords,
				open: () => viaShortcut(a.id)
			});
		}
		out.push(
			{
				id: 'do-next',
				group: 'Actions',
				icon: 'play',
				title: 'Do next',
				sub: 'the top project’s next step',
				hint: '⌘⇧↵',
				open: () => doNext().then(report)
			},
			{
				id: 'new',
				group: 'Actions',
				icon: 'plus',
				title: 'New project',
				sub: '',
				hint: '⌘N',
				open: () => goto(to.setup())
			},
			{
				id: 'drawer',
				group: 'Actions',
				icon: 'drawer',
				title: 'Toggle terminal drawer',
				sub: '',
				hint: '⌃`',
				open: () => ui.toggleDrawer()
			},
			{
				id: 'run-log',
				group: 'Actions',
				icon: 'log',
				title: 'Open the run log',
				sub: '',
				hint: '',
				open: () => goto(to.runLog())
			},
			{
				id: 'doctor',
				group: 'Actions',
				icon: 'pulse',
				title: 'Run Doctor',
				sub: '',
				hint: '',
				open: () => goto(to.doctor())
			},
			{
				id: 'health',
				group: 'Actions',
				icon: 'file',
				title: 'Template health',
				sub: '',
				hint: '',
				open: () => goto(to.templateHealth())
			},
			{
				id: 'settings',
				group: 'Actions',
				icon: 'sliders',
				title: 'Settings',
				sub: '',
				hint: '⌘,',
				open: () => goto(to.settings())
			}
		);
		for (const v of views)
			out.push({
				id: `proj-${v.summary.id}`,
				group: 'Projects',
				icon: 'folder',
				title: v.summary.name,
				sub: `${v.summary.displayPath}, Stage ${v.summary.currentStage ?? 0}`,
				hint: v.summary.pinned ? `⌘${v.summary.pinned}` : '',
				open: () => goto(to.project(v.summary.id))
			});
		if (proj)
			for (const s of proj.stages)
				out.push({
					id: `stage-${s.stage}`,
					group: 'Stages',
					icon: 'file',
					title: `${s.stage} ${s.title}`,
					sub: `${proj.summary.name}, ${s.status === 'inProgress' ? 'in progress' : s.status === 'needsAnnotation' ? 'needs annotation' : s.status === 'notApplicable' ? 'not applicable' : s.status}`,
					hint: '',
					open: () => goto(to.stage(proj.summary.id, s.stage))
				});
		return out;
	});

	// Each group is capped so a long group never pushes the others out of the list; with no query
	// the Steps group is the open stage's steps.
	const CAP = { Steps: 12, Actions: 30, Projects: 12, Stages: 19 } as const;
	const shown: Item[] = $derived.by(() => {
		const q = query.trim().toLowerCase();
		const stageNo = current.stage ?? current.project?.summary.currentStage ?? null;
		const pid = current.project?.summary.id ?? null;
		const list = q
			? items.filter((i) => `${i.title} ${i.sub} ${i.keywords ?? ''}`.toLowerCase().includes(q))
			: items.filter(
					(i) =>
						i.group !== 'Steps' || (stageNo !== null && i.projectId === pid && i.stage === stageNo)
				);
		const seen: Record<string, number> = {};
		return list.filter((i) => (seen[i.group] = (seen[i.group] ?? 0) + 1) <= CAP[i.group]);
	});
	const groups = $derived(
		(['Steps', 'Actions', 'Projects', 'Stages'] as const)
			.map((g) => ({ g, list: shown.filter((i) => i.group === g) }))
			.filter((x) => x.list.length > 0)
	);
	const flat = $derived(groups.flatMap((x) => x.list));
	const activeId = $derived(flat[active]?.id ?? '');

	async function choose(i: Item | undefined, run: boolean): Promise<void> {
		if (!i) return;
		ui.paletteOpen = false;
		if (run && i.run) await i.run();
		else await i.open();
	}

	function onKey(e: KeyboardEvent): void {
		if (e.key === 'Escape') {
			e.preventDefault();
			close();
		} else if (e.key === 'ArrowDown') {
			e.preventDefault();
			active = Math.min(flat.length - 1, active + 1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			active = Math.max(0, active - 1);
		} else if (e.key === 'Enter') {
			e.preventDefault();
			void choose(flat[active], e.metaKey);
		} else if (e.key === 'Tab') e.preventDefault();
	}
</script>

{#if ui.paletteOpen}
	<div class="scrim" aria-hidden="true" onclick={close}></div>
	<div class="palette" role="dialog" aria-modal="true" aria-label="Commands">
		<div class="palette-input">
			<Icon name="search" />
			<label class="vh" for="pal-q">Search projects, stages, steps and actions</label>
			<input
				id="pal-q"
				bind:this={input}
				bind:value={query}
				role="combobox"
				aria-expanded="true"
				aria-controls="pal-list"
				aria-activedescendant={activeId || undefined}
				aria-autocomplete="list"
				autocomplete="off"
				spellcheck="false"
				oninput={() => (active = 0)}
				onkeydown={onKey}
			/>
			<kbd>esc</kbd>
		</div>
		<div id="pal-list" role="listbox" aria-label="Results">
			{#each groups as grp (grp.g)}
				<div class="palette-group" role="group" aria-labelledby="pg-{grp.g}">
					<div class="group-label" id="pg-{grp.g}">{grp.g}</div>
					{#each grp.list as i (i.id)}
						<!-- svelte-ignore a11y_click_events_have_key_events (keyboard goes through the combobox) -->
						<div
							class="palette-item"
							role="option"
							id={i.id}
							aria-selected={i.id === activeId}
							tabindex="-1"
							onclick={() => choose(i, false)}
						>
							<Icon name={i.icon} />
							<span
								>{i.title}
								{#if i.sub}<span class="sub">{i.sub}</span>{/if}</span
							>
							{#if i.hint && (i.group !== 'Steps' || i.id === activeId)}<kbd>{i.hint}</kbd
								>{:else}<span></span>{/if}
						</div>
					{/each}
				</div>
			{:else}
				<p class="none">No steps or actions match “{query}”. Try a step number like 2.4.</p>
			{/each}
		</div>
		<div class="palette-foot">
			<span>↑↓ move</span><span>↵ open</span><span>⌘↵ run</span><span>⌘G jump to a stage</span>
		</div>
	</div>
{/if}

<style>
	.palette-input {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: 0 var(--space-5);
		height: 48px;
		border-bottom: var(--hairline) solid var(--rule);
	}
	.palette-input input {
		flex: 1;
		border: 0;
		background: transparent;
		font-size: var(--text-15);
		color: var(--ink);
		outline: none;
	}
	#pal-list {
		max-height: min(60vh, 520px);
		overflow: auto;
	}
	.palette-group {
		padding: var(--space-3) var(--space-3) var(--space-2);
	}
	.group-label {
		font-size: var(--text-11);
		color: var(--ink-muted);
		padding: var(--space-1) var(--space-3) var(--space-2);
		font-weight: var(--weight-semibold);
	}
	.palette-item {
		display: grid;
		grid-template-columns: 20px minmax(0, 1fr) auto;
		gap: var(--space-3);
		align-items: center;
		padding: 6px var(--space-3);
		border-radius: var(--radius-control);
	}
	.palette-item[aria-selected='true'] {
		background: var(--selected);
	}
	.sub {
		color: var(--ink-muted);
		font-size: var(--text-12);
	}
	.palette-item :global(.icon) {
		color: var(--ink-muted);
	}
	.palette-foot {
		display: flex;
		gap: var(--space-5);
		padding: 7px var(--space-5);
		border-top: var(--hairline) solid var(--rule);
		font-size: var(--text-11);
		color: var(--ink-muted);
	}
	.none {
		padding: var(--space-5);
		color: var(--ink-muted);
		font-size: var(--text-13);
	}
</style>
