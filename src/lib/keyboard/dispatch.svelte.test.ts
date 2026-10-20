import { createRawSnippet, tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { goto } from '$app/navigation';
import { to } from '#lib/nav/routes.ts';
import { current } from '#lib/stores/project.svelte.ts';
import { terminal } from '#lib/stores/terminal.svelte.ts';
import { ui } from '#lib/stores/ui.svelte.ts';
import { fresh, IDS, said } from '#lib/test/screen.ts';

import ProjectLayout from '../../routes/projects/[projectId]/+layout.svelte';
import RootLayout from '../../routes/+layout.svelte';

const route = vi.hoisted(() => ({
	url: new URL('http://localhost/projects/x/stages/2'),
	params: { projectId: '', stage: '2' }
}));
vi.mock('$app/state', () => ({ page: route }));

// M4-33 (AC-F028-04, AC-F028-05): the remaining A.7 shortcuts each invoke their action, from the
// window's dispatcher, with a one-line reason when they don't apply (AC-F028-03).
const page = createRawSnippet(() => ({
	render: () => '<main id="main"><h1>Architect’s Design Studio</h1><p>body</p></main>'
}));

function press(k: string, mods: Partial<KeyboardEventInit> = {}, target: EventTarget = window) {
	target.dispatchEvent(
		new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...mods })
	);
}
const cmd = (k: string, mods: Partial<KeyboardEventInit> = {}, target?: EventTarget) =>
	press(k, { metaKey: true, ...mods }, target);

async function root() {
	const m = fresh('happy');
	route.params = { projectId: IDS.tradelog, stage: '2' };
	ui.paletteOpen = false;
	ui.drawerOpen = false;
	ui.chrome = { title: 'Foreman', sub: '', drawer: false, inspector: false };
	const s = await render(RootLayout, { children: page });
	await expect.element(s.getByRole('heading', { level: 1 })).toBeInTheDocument();
	return { m, s };
}

afterEach(() => {
	ui.paletteOpen = false;
	current.project = null;
	current.step = null;
});

describe('shortcuts through the window dispatcher', () => {
	it('⌘K opens the palette, from anywhere including the terminal (AC-F028-04)', async () => {
		await root();
		const term = document.createElement('div');
		term.className = 'xterm';
		document.body.append(term);
		cmd('k', {}, term);
		expect(ui.paletteOpen).toBe(true);
		term.remove();
	});

	it('⌘N and ⌘, open New project and Settings (AC-F028-05)', async () => {
		await root();
		cmd('n');
		await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(to.setup()));
		cmd(',');
		await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(to.settings()));
	});

	it('⌘1 opens the project pinned to 1; an unpinned number says so', async () => {
		await root();
		cmd('1');
		await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(to.project(IDS.tradelog)));
		cmd('9');
		await vi.waitFor(() => expect(said()).toContain('No project is pinned to that number.'));
	});

	it('⌃` opens the terminal drawer', async () => {
		await root();
		press('`', { ctrlKey: true, code: 'Backquote' });
		await vi.waitFor(() => expect(ui.drawerOpen).toBe(true));
	});

	it('⌘⌥I toggles the inspector where there is one and says so where there is not', async () => {
		await root();
		cmd('ˆ', { altKey: true, code: 'KeyI' });
		await vi.waitFor(() => expect(said()).toContain('This screen has no inspector.'));
		ui.chrome = { title: 'Foreman', sub: '', drawer: false, inspector: true };
		const before = [ui.inspectorOpen, ui.inspectorOverlay];
		cmd('ˆ', { altKey: true, code: 'KeyI' });
		await vi.waitFor(() => expect([ui.inspectorOpen, ui.inspectorOverlay]).not.toEqual(before));
	});

	it('⌘⇧T toggles focus mode with a terminal session and needs one otherwise', async () => {
		await root();
		await vi.waitFor(() => expect(terminal.tabs.length).toBeGreaterThan(0));
		cmd('T', { shiftKey: true });
		await vi.waitFor(() => expect(terminal.focusMode).toBe(true));
		terminal.focusMode = false;
		terminal.reset();
		await tick();
		cmd('T', { shiftKey: true });
		await vi.waitFor(() => expect(said()).toContain('Focus mode needs a terminal session.'));
	});

	it('⌘⇧E returns focus to the workspace', async () => {
		await root();
		cmd('E', { shiftKey: true });
		await vi.waitFor(() => expect(document.activeElement?.tagName).toBe('H1'));
	});

	it('⌘⇧↵ does the next thing from anywhere', async () => {
		const { m } = await root();
		const next = vi.spyOn(m, 'doNext');
		cmd('Enter', { shiftKey: true });
		await vi.waitFor(() => expect(next).toHaveBeenCalled());
	});

	describe('inside a project', () => {
		async function inProject(stage: string) {
			const out = await root();
			route.params = { projectId: IDS.tradelog, stage };
			const p = await render(ProjectLayout, { children: page });
			return { ...out, p };
		}

		it('⌘] and ⌘[ move one stage; the ends say so', async () => {
			await inProject('2');
			cmd(']');
			await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(to.stage(IDS.tradelog, 3)));
			cmd('[');
			await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(to.stage(IDS.tradelog, 1)));
		});

		it('⌘] on the last stage and ⌘[ on the first say there is no more', async () => {
			const { p } = await inProject('18');
			cmd(']');
			await vi.waitFor(() => expect(said()).toContain('This is the last stage.'));
			await p.unmount();
			route.params = { projectId: IDS.tradelog, stage: '0' };
			await render(ProjectLayout, { children: page });
			cmd('[');
			await vi.waitFor(() => expect(said()).toContain('This is the first stage.'));
		});

		it('⌘G then a number jumps to that stage', async () => {
			await inProject('2');
			cmd('g');
			press('7');
			await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(to.stage(IDS.tradelog, 7)));
			cmd('g');
			press('1');
			press('2');
			await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(to.stage(IDS.tradelog, 12)));
		});

		it('⌘⇧G opens the gate for this stage', async () => {
			await inProject('2');
			cmd('G', { shiftKey: true });
			await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(to.gate(IDS.tradelog, 2)));
		});
	});

	it('outside a project the stage shortcuts say what to do instead of acting', async () => {
		await root();
		cmd(']');
		await vi.waitFor(() => expect(said()).toContain('Open a project to move between stages.'));
		cmd('G', { shiftKey: true });
		await vi.waitFor(() => expect(said()).toContain('Open a project to see its gate.'));
	});
});
