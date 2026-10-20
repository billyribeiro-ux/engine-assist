// Component test in browser mode against MockIpcClient (ADR-015 layer 3, M0-05 example).
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';

import Page from './+page.svelte';
import { load } from './+page.ts';

describe('Today (placeholder)', () => {
	it('renders the first-run state that app_bootstrap reports through the mock', async () => {
		const data = await load({} as Parameters<typeof load>[0]);
		await render(Page, { data } as never);
		await expect.element(page.getByRole('heading', { level: 1 })).toHaveTextContent('Today');
		await expect.element(page.getByText('Welcome to Foreman.', { exact: false })).toBeVisible();
	});
});
