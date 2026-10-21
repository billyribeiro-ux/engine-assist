<script lang="ts">
	// A step opened directly (palette, Today, a chat step's link): the stage workspace with it selected.
	import { page } from '$app/state';

	import StageWorkspace from '#lib/screens/StageWorkspace.svelte';
	import { current } from '#lib/stores/project.svelte.ts';
</script>

{#if current.project}
	<StageWorkspace
		project={current.project}
		stageNo={Number(page.params.stage)}
		stepKey={page.params.step ?? null}
	/>
{/if}
