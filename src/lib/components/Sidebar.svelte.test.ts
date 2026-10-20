import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { fresh } from '#lib/test/screen.ts';

import Sidebar from './Sidebar.svelte';

// M4-25 (AC-F017-10): every sidebar entry shows a status dot, as a shape plus words, and a pinned
// project shows its ⌘ number.
describe('Sidebar status', () => {
	it('primary: each project carries a status with words, and pinned ones their number', async () => {
		fresh('happy');
		const s = await render(Sidebar);
		const projects = s.getByRole('list', { name: 'Projects' });
		await expect.element(projects).toBeVisible();
		const rows = [...projects.element().querySelectorAll('li > a.side-item')];
		expect(rows.length).toBeGreaterThan(1);
		for (const a of rows) {
			const status = a.querySelector('.badge[role="img"]');
			expect(status?.getAttribute('aria-label'), a.textContent).toMatch(
				/^(No problems|Waiting on you|Has a problem|Read-only)$/
			);
		}
		const pins = [...projects.element().querySelectorAll('.pin')].map((e) => e.textContent);
		expect(pins).toContain('⌘1');
	});

	it('a project with a failed packet shows a problem and one with a signable gate shows waiting', async () => {
		fresh('happy');
		const s = await render(Sidebar);
		await expect.element(s.getByRole('img', { name: 'Has a problem' }).first()).toBeInTheDocument();
		await expect
			.element(s.getByRole('img', { name: 'Waiting on you' }).first())
			.toBeInTheDocument();
	});

	it('empty: no projects says so and shows no status', async () => {
		fresh('empty');
		const s = await render(Sidebar);
		await expect.element(s.getByText('No projects yet')).toBeVisible();
		await expect.element(s.getByRole('img', { name: 'No problems' })).not.toBeInTheDocument();
	});
});
