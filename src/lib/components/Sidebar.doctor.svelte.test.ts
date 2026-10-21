import { createRawSnippet } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { fresh } from '#lib/test/screen.ts';

import RootLayout from '../../routes/+layout.svelte';
import Sidebar from './Sidebar.svelte';

const route = vi.hoisted(() => ({
	url: new URL('http://localhost/'),
	params: {}
}));
vi.mock('$app/state', () => ({ page: route }));

// M2-15 (AC-F019-07): Doctor runs by itself when Foreman starts, and the sidebar's dot shows its
// status, as a shape and words.
const page = createRawSnippet(() => ({
	render: () => '<main id="main"><h1>Today</h1></main>'
}));

describe('Doctor status dot (AC-F019-07)', () => {
	it('Foreman starting runs Doctor in the background', async () => {
		const m = fresh('happy');
		const run = vi.spyOn(m, 'doctorRun');
		await render(RootLayout, { children: page });
		await vi.waitFor(() => expect(run).toHaveBeenCalledWith({ scope: 'p1' }));
	});

	it('the dot follows the status Doctor reports', async () => {
		const m = fresh('happy');
		const s = await render(Sidebar);
		await m.doctorRun({ scope: 'p1' });
		await vi.waitFor(() =>
			expect(
				s.container.querySelector('a[href$="/doctor"] .badge')?.getAttribute('aria-label')
			).toMatch(/^Doctor: /)
		);
	});
});
