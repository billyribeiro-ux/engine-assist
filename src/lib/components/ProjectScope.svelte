<script lang="ts">
	// Loads a project (project_open) and keeps it current: the strip, sidebar, title bar and every
	// project screen read it. Refetches on `project:changed` (with a newer rev) and on packet events.
	// A project whose state can't be read offers Rebuild from repo (F-020).
	import type { Snippet } from 'svelte';

	import { failureText } from '#lib/actions/words.ts';
	import type { ProjectView } from '#lib/ipc/types.ts';
	import { current } from '#lib/stores/project.svelte.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';

	import Icon from './Icon.svelte';
	import RebuildFromRepo from './RebuildFromRepo.svelte';

	let { projectId, children }: { projectId: string; children: Snippet<[ProjectView]> } = $props();

	const project = resource(
		() => projectId,
		(c, id) => c.projectOpen({ projectId: id }),
		(id) => [`project:${id}`, 'projects']
	);
	$effect(() => {
		current.project = project.value?.summary.id === projectId ? project.value : null;
	});
	$effect(() => () => {
		current.project = null;
	});
</script>

{#if project.value?.summary.id === projectId}
	{@render children(project.value)}
{:else if project.error}
	<main class="main" id="main">
		<div class="page">
			<h1 class="page-title">Project not available</h1>
			<p class="err" role="alert"><Icon name="cross" size="glyph" />{failureText(project.error)}</p>
			{#if project.error.code === 'io'}
				<!-- AC-F020-12: unreadable state is kept; the rebuild is reviewed, then confirmed. -->
				<RebuildFromRepo {projectId} onrebuilt={(v: ProjectView) => project.set(v)} />
			{/if}
		</div>
	</main>
{/if}
