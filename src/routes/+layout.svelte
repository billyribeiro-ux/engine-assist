<script lang="ts">
	// The window (components.md "Layout"): sidebar, title bar, the routed screen, the terminal drawer,
	// the palette, toasts and the shared live region. Keyboard shortcuts are dispatched here (A.7).
	import '#lib/styles/tokens.css';
	import '#lib/styles/base.css';

	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import type { Snippet } from 'svelte';
	import { onMount } from 'svelte';

	import { doNext, report } from '#lib/actions/run.ts';
	import Palette from '#lib/components/Palette.svelte';
	import Sidebar from '#lib/components/Sidebar.svelte';
	import TerminalDrawer from '#lib/components/TerminalDrawer.svelte';
	import Titlebar from '#lib/components/Titlebar.svelte';
	import Toasts from '#lib/components/Toasts.svelte';
	import { ipc } from '#lib/ipc/index.ts';
	import { handlerFor, isTyping, match, NOT_HERE, registerShortcuts } from '#lib/keyboard/registry.ts';
	import { to } from '#lib/nav/routes.ts';
	import { refresh } from '#lib/stores/bus.svelte.ts';
	import { current } from '#lib/stores/project.svelte.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { terminal } from '#lib/stores/terminal.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	let { children }: { children: Snippet } = $props();

	const settings = resource(
		() => null,
		(c) => c.settingsGet({}),
		() => ['settings']
	);
	$effect(() => {
		if (settings.value) ui.applyTheme(settings.value.theme);
	});

	let drawer = $state<TerminalDrawer>();
	let forced = $state(false);
	const showDrawer = $derived(ui.chrome.drawer || forced);
	$effect(() => {
		void page.url.pathname;
		forced = false;
	});

	onMount(() => {
		void (async () => {
			const client = await ipc();
			const boot = await client.appBootstrap({});
			if (boot.firstRun && page.url.pathname === '/') await goto(to.firstRun());
			await client.listen('run:status', (p) => {
				terminal.setStatus(p.runId, p.status, p.status === 'exited' ? 0 : null);
			});
			const runs = await client.runsSearch({ query: '', projectId: null, cursor: null, limit: 50 });
			for (const r of runs.runs.filter((x) => ['running', 'idle', 'blocked', 'starting'].includes(x.status))) {
				const before = runs.runs.find((x) => x.chain === r.chain && x.runId !== r.runId && x.startedAt < r.startedAt);
				await terminal.attach(client, r.runId, r.project, r.chain);
				if (before?.step)
					terminal.tabs = terminal.tabs.map((t) =>
						t.runId === r.runId ? { ...t, archived: { label: `${before.step ?? ''} ${before.stepTitle}`, lines: 1842 } } : t
					);
			}
		})();

		// Reduce Transparency and Increase Contrast are reported to Rust, which turns the window's
		// vibrancy off (ADR-014, AC-XC-23). The value isn't stored.
		const rt = matchMedia('(prefers-reduced-transparency: reduce)');
		const hc = matchMedia('(prefers-contrast: more)');
		const report_ = () => {
			void ipc().then(async (c) => {
				const s = await c.settingsGet({});
				await c
					.settingsUpdate({
						expectRev: s.rev,
						patch: {
							driName: null,
							editor: null,
							theme: null,
							terminal: null,
							envPassthrough: null,
							globalVariables: null,
							resetTool: null,
							clearLoginPath: null,
							removeAllowlistEntry: null,
							screenReaderMode: null,
							osDisplay: { reduceTransparency: rt.matches, increaseContrast: hc.matches }
						}
					})
					.catch(() => undefined);
			});
		};
		report_();
		rt.addEventListener('change', report_);
		hc.addEventListener('change', report_);

		const off = registerShortcuts({
			palette: () => {
				ui.paletteOpen = true;
				return true;
			},
			toggleDrawer: () => {
				if (!ui.chrome.drawer) {
					forced = !forced || !ui.drawerOpen;
					if (forced && !ui.drawerOpen) ui.toggleDrawer();
					return true;
				}
				ui.toggleDrawer();
				return true;
			},
			newProject: () => {
				void goto(to.setup());
				return true;
			},
			settings: () => {
				void goto(to.settings());
				return true;
			},
			doNext: () => doNext(),
			returnFocus: () => {
				const main = document.getElementById('main');
				const target = main?.querySelector<HTMLElement>('[aria-current="step"] button, [aria-current="step"], h1') ?? main;
				if (target && !target.hasAttribute('tabindex') && target.tagName === 'H1') target.setAttribute('tabindex', '-1');
				target?.focus();
				return true;
			},
			focusMode: () => {
				if (terminal.tabs.length === 0) return NOT_HERE.focusMode;
				terminal.focusMode = !terminal.focusMode;
				forced = true;
				if (!ui.drawerOpen) ui.toggleDrawer();
				drawer?.focusTerminal();
				return true;
			},
			pinned: async (slot) => {
				const c = await ipc();
				const p = (await c.projectsList({})).find((x) => x.pinned === slot);
				if (!p) return NOT_HERE.pinned;
				await goto(to.project(p.id));
				return true;
			},
			toggleInspector: () => {
				if (!ui.chrome.inspector) return NOT_HERE.toggleInspector;
				ui.toggleInspector();
				return true;
			}
		});
		return () => {
			off();
			rt.removeEventListener('change', report_);
			hc.removeEventListener('change', report_);
		};
	});

	let jump: { digits: string; timer: ReturnType<typeof setTimeout> } | null = null;

	function onKeydown(e: KeyboardEvent): void {
		if (ui.paletteOpen && !(e.metaKey && e.key.toLowerCase() === 'k')) return;
		if (jump && /^\d$/.test(e.key) && !e.metaKey) {
			e.preventDefault();
			clearTimeout(jump.timer);
			jump.digits += e.key;
			const d = jump.digits;
			const go = () => {
				jump = null;
				const h = handlerFor('jumpStage');
				if (h) void Promise.resolve(h(Number(d))).then(report);
			};
			if (d.length >= 2 || Number(d) > 1) go();
			else jump.timer = setTimeout(go, 700);
			return;
		}
		const m = match(e, !isTyping(e.target) && !e.defaultPrevented);
		if (!m) return;
		if (m.id === 'jumpStage') {
			e.preventDefault();
			if (!handlerFor('jumpStage')) return report(NOT_HERE.jumpStage);
			jump = { digits: '', timer: setTimeout(() => (jump = null), 1500) };
			ui.announce('Jump to stage: type the number');
			return;
		}
		const h = handlerFor(m.id);
		if (!h) {
			if (NOT_HERE[m.id]) {
				e.preventDefault();
				report(NOT_HERE[m.id]);
			}
			return;
		}
		e.preventDefault();
		void Promise.resolve(h(m.arg)).then(report);
	}

	// Refresh everything when the window comes back (re-stat on focus, ipc.md §6).
	function onFocus(): void {
		refresh('today', 'projects', 'doctor');
		if (current.project) refresh(`project:${current.project.summary.id}`);
	}
</script>

<svelte:window onkeydown={onKeydown} onfocus={onFocus} />

<div class="app">
	<a class="skip-link btn" href="#main">Skip to content</a>
	<Sidebar />
	<div class="window">
		<Titlebar />
		{@render children()}
		{#if showDrawer}
			<TerminalDrawer
				bind:this={drawer}
				projectName={current.project?.summary.name ?? null}
				fontSize={settings.value?.terminal.fontSize ?? 12}
				scrollback={settings.value?.terminal.scrollback ?? 10000}
				screenReaderMode={settings.value?.screenReaderMode ?? false}
			/>
		{/if}
	</div>
</div>
<Palette />
<Toasts />
