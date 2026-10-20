<script lang="ts">
	// Knowledge manifest (F-023, AC-F023-01, -02; mockup workspace-chat.html "Knowledge for this
	// chat"): the signed packets and chat outputs that belong in the claude.ai Project, each with a
	// checkbox ("in the Project"), Reveal in Finder and Copy contents. With nothing yet it points to
	// the first step that produces a file. The checkbox is Billy's own note; nothing is uploaded.
	import { failureText } from '#lib/actions/words.ts';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import type { Manifest } from '#lib/ipc/types.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import Icon from './Icon.svelte';

	let {
		projectId,
		manifest,
		onchange
	}: {
		projectId: string;
		manifest: Manifest;
		onchange: (m: Manifest) => void;
	} = $props();

	const id = (path: string) => `km-${path.replace(/[^a-z0-9]/gi, '-')}`;

	async function mark(path: string, inProject: boolean): Promise<void> {
		const c = await ipc();
		try {
			onchange(await c.manifestMark({ projectId, path, inProject }));
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	async function reveal(path: string): Promise<void> {
		const c = await ipc();
		try {
			await c.fileReveal({ projectId, path });
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
	async function copy(path: string): Promise<void> {
		const c = await ipc();
		try {
			await c.clipboardCopy({
				projectId,
				target: 'fileContents',
				step: null,
				chain: null,
				path,
				stage: null
			});
			ui.toast(`Contents of ${path.split('/').at(-1) ?? path} copied`, 'pass', 'copy');
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}
</script>

<div class="km">
	<h2 class="insp-title" id="km-title">Knowledge for this chat</h2>
	{#if manifest.files.length === 0}
		<p class="insp-sub">
			{manifest.firstProducer
				? `Nothing to upload yet. ${manifest.firstProducer} writes the first file for the claude.ai Project; it appears here once its packet is signed.`
				: 'Nothing to upload yet. Signed packets appear here for the claude.ai Project.'}
		</p>
	{:else}
		<p class="insp-sub" role="status">
			Knowledge ready: {manifest.ready} of {manifest.total} in the claude.ai Project.
		</p>
		<ul class="insp-list" aria-labelledby="km-title">
			{#each manifest.files as f (f.path)}
				<li>
					<input
						type="checkbox"
						class="check"
						id={id(f.path)}
						checked={f.inProject}
						disabled={!f.exists}
						aria-describedby="{id(f.path)}-kind"
						onchange={(e) => mark(f.path, e.currentTarget.checked)}
					/>
					<span>
						<label for={id(f.path)}><code>{f.path}</code></label>
						<span class="kind" id="{id(f.path)}-kind"
							>{!f.exists
								? `Not written yet${f.producer ? ` (${f.producer})` : ''}`
								: f.inProject
									? 'In the claude.ai Project'
									: 'Upload to the claude.ai Project'}</span
						>
						{#if f.exists}
							<span class="row-actions">
								<button
									class="btn ghost small"
									type="button"
									aria-label="Reveal {f.path} in Finder"
									onclick={() => reveal(f.path)}><Icon name="folder" />Reveal</button
								>
								<button
									class="btn ghost small"
									type="button"
									aria-label="Copy the contents of {f.path}"
									onclick={() => copy(f.path)}><Icon name="copy" />Copy contents</button
								>
							</span>
						{/if}
					</span>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.check {
		width: 16px;
		height: 16px;
		margin: 2px 0 0;
		accent-color: var(--accent);
	}
	.row-actions {
		display: flex;
		gap: var(--space-2);
		margin-top: var(--space-1);
	}
	.small {
		font-size: var(--text-12);
	}
	label code {
		overflow-wrap: anywhere;
	}
</style>
