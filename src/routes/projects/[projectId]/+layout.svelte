<script lang="ts">
	// A project (F-030): the sheet strip over every project screen, and the stage shortcuts
	// ⌘] / ⌘[ (next / previous stage), ⌘G then a number (jump) and ⌘⇧G (gate for this stage).
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import type { Snippet } from 'svelte';
	import { onMount } from 'svelte';

	import ProjectScope from '#lib/components/ProjectScope.svelte';
	import SheetStrip from '#lib/components/SheetStrip.svelte';
	import type { ProjectView } from '#lib/ipc/types.ts';
	import { registerShortcuts } from '#lib/keyboard/registry.ts';
	import { to } from '#lib/nav/routes.ts';
	import { markSwitchEnd, markSwitchStart } from '#lib/perf.ts';
	import { current } from '#lib/stores/project.svelte.ts';

	let { children }: { children: Snippet } = $props();

	const projectId = $derived(page.params.projectId ?? '');
	const stageParam = $derived(page.params.stage === undefined ? null : Number(page.params.stage));

	// NFR-02: the switch starts when the route's project changes and ends once its screen is up.
	$effect(() => {
		if (projectId && current.project?.summary.id !== projectId) markSwitchStart(projectId);
	});
	$effect(() => {
		if (projectId && current.project?.summary.id === projectId) markSwitchEnd(projectId);
	});

	onMount(() =>
		registerShortcuts({
			nextStage: () => step(1),
			prevStage: () => step(-1),
			jumpStage: (n) => {
				if (n === undefined || n > 18) return 'There are 19 stages, 0 to 18.';
				void goto(to.stage(projectId, n));
				return true;
			},
			openGate: () => {
				const s = stageParam ?? current.project?.summary.currentStage ?? 0;
				void goto(to.gate(projectId, s));
				return true;
			}
		})
	);

	function step(d: number): true | string {
		const s = (stageParam ?? current.project?.summary.currentStage ?? 0) + d;
		if (s < 0 || s > 18) return d > 0 ? 'This is the last stage.' : 'This is the first stage.';
		void goto(to.stage(projectId, s));
		return true;
	}
</script>

<ProjectScope {projectId}>
	{#snippet children(p: ProjectView)}
		<SheetStrip
			{projectId}
			projectName={p.summary.name}
			stages={p.stages}
			open={stageParam ?? p.summary.currentStage}
		/>
		{@render body()}
	{/snippet}
</ProjectScope>

{#snippet body()}
	{@render children()}
{/snippet}
