import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import type { Manifest } from '#lib/ipc/types.ts';
import { fresh, IDS } from '#lib/test/screen.ts';

import KnowledgeManifest from './KnowledgeManifest.svelte';

// M4-27 (F-023): the knowledge manifest against the mock.
const packet = {
	path: 'docs/handoff/stage-01.md',
	exists: true,
	inProject: false,
	producer: '1.4'
};
const teardown = { path: 'spec/02-teardown.md', exists: false, inProject: false, producer: '2.1' };
const two: Manifest = {
	files: [packet, teardown],
	ready: 0,
	total: 2,
	firstProducer: null
};

describe('KnowledgeManifest', () => {
	it('AC-F023-01: each file has a checkbox, Reveal in Finder and Copy contents', async () => {
		const m = fresh('stage4');
		const reveal = vi.spyOn(m, 'fileReveal');
		const copy = vi.spyOn(m, 'clipboardCopy');
		const mark = vi.spyOn(m, 'manifestMark').mockResolvedValue({
			...two,
			files: [{ ...packet, inProject: true }, teardown],
			ready: 1
		});
		let current = two;
		const s = await render(KnowledgeManifest, {
			projectId: IDS.tradelog,
			manifest: two,
			onchange: (x: Manifest) => (current = x)
		});
		await expect
			.element(s.getByText('Knowledge ready: 0 of 2 in the claude.ai Project.'))
			.toBeVisible();
		const box = s.getByRole('checkbox', { name: 'docs/handoff/stage-01.md' });
		await expect.element(box).not.toBeChecked();
		await box.click();
		await vi.waitFor(() =>
			expect(mark).toHaveBeenCalledWith({
				projectId: IDS.tradelog,
				path: 'docs/handoff/stage-01.md',
				inProject: true
			})
		);
		await vi.waitFor(() => expect(current.ready).toBe(1));
		await s.getByRole('button', { name: 'Reveal docs/handoff/stage-01.md in Finder' }).click();
		await vi.waitFor(() =>
			expect(reveal).toHaveBeenCalledWith({
				projectId: IDS.tradelog,
				path: 'docs/handoff/stage-01.md'
			})
		);
		await s.getByRole('button', { name: 'Copy the contents of docs/handoff/stage-01.md' }).click();
		await vi.waitFor(() =>
			expect(copy).toHaveBeenCalledWith(
				expect.objectContaining({ target: 'fileContents', path: 'docs/handoff/stage-01.md' })
			)
		);
		// A declared output not written yet can't be marked, revealed or copied.
		await expect.element(s.getByRole('checkbox', { name: 'spec/02-teardown.md' })).toBeDisabled();
		await expect.element(s.getByText('Not written yet (2.1)')).toBeVisible();
		await expect
			.element(s.getByRole('button', { name: 'Reveal spec/02-teardown.md in Finder' }))
			.not.toBeInTheDocument();
	});

	it('AC-F023-02: with nothing yet it points to the first step that produces a file', async () => {
		fresh('happy');
		const s = await render(KnowledgeManifest, {
			projectId: IDS.tradelog,
			manifest: { files: [], ready: 0, total: 0, firstProducer: '1.4' },
			onchange: (x: Manifest) => {
				expect(x).toBeNull();
			}
		});
		await expect
			.element(s.getByText(/Nothing to upload yet\. 1\.4 writes the first file/))
			.toBeVisible();
		await expect.element(s.getByRole('checkbox')).not.toBeInTheDocument();
	});
});
