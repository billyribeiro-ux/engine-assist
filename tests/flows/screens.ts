// The screens every whole-app flow walks: one entry per mockup in docs/design/mockups/, with the mock
// world's route that fills it. Shared by visual.spec.ts (AC-XC-26), status.spec.ts (AC-XC-25) and
// layout checks, so a new screen is added once.
import type { Page } from '@playwright/test';

export const TL = '7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11';
export const SW = '5e1f2a3b-4c5d-4e6f-8a7b-9c0d1e2f3a4b';
export const FM = 'f0e1d2c3-b4a5-4697-8879-6a5b4c3d2e1f';

export interface Screen {
	/** The mockup's file name without `.html`, plus `-<state>` for a state the mockup shows by `#hash`. */
	id: string;
	mock: string;
	hash?: string;
	/** The route in the mock world. */
	url: string;
	/** Opens the state after the page loads (a dialog, the palette). */
	open?: (page: Page) => Promise<void>;
}

export const SCREENS: Screen[] = [
	{ id: 'today', mock: 'today', url: '/' },
	{ id: 'first-run', mock: 'first-run', url: '/first-run?scenario=firstRun' },
	{ id: 'setup', mock: 'setup', url: '/setup' },
	{ id: 'wizard', mock: 'wizard', url: '/setup/wizard' },
	{ id: 'workspace', mock: 'workspace', url: `/projects/${TL}/stages/2` },
	{ id: 'workspace-chat', mock: 'workspace-chat', url: `/projects/${TL}/stages/1?scenario=stage1` },
	{ id: 'gate', mock: 'gate', url: `/projects/${TL}/stages/5/gate?scenario=stage5` },
	{
		id: 'gate-sign',
		mock: 'gate',
		hash: 'sign',
		url: `/projects/${SW}/stages/7/gate`,
		open: async (page) => {
			await page.getByRole('button', { name: 'Sign packet' }).click();
			await page.getByRole('dialog').waitFor();
		}
	},
	{
		id: 'packet',
		mock: 'packet',
		url: `/projects/${TL}/stages/2/packet?scenario=invalidPacket`
	},
	{
		id: 'outside-change',
		mock: 'outside-change',
		url: `/projects/${TL}/changes?scenario=outsideChange`
	},
	{ id: 'assembler', mock: 'assembler', url: `/projects/${TL}/assembler?scenario=stage4` },
	{ id: 'doctor', mock: 'doctor', url: '/doctor' },
	{ id: 'run-log', mock: 'run-log', url: '/run-log' },
	{ id: 'settings', mock: 'settings', url: '/settings' },
	{ id: 'template-health', mock: 'template-health', url: '/template-health' },
	{
		id: 'palette',
		mock: 'palette',
		url: `/projects/${TL}/stages/2`,
		open: async (page) => {
			await page.keyboard.press('Meta+k');
			await page.getByRole('dialog').waitFor();
		}
	}
];

export const THEMES = [
	{ name: 'night', scheme: 'dark' },
	{ name: 'vellum', scheme: 'light' }
] as const;

/** The two sizes the mockups are shot at. */
export const VIEWPORTS = [
	{ width: 1280, height: 800 },
	{ width: 1440, height: 900 }
] as const;
