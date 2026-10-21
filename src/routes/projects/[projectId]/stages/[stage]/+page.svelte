<script lang="ts">
	import { page } from '$app/state';

	import StageWorkspace from '#lib/screens/StageWorkspace.svelte';
	import { current } from '#lib/stores/project.svelte.ts';
</script>

{#if current.project}
	<StageWorkspace project={current.project} stageNo={Number(page.params.stage)} />
{/if}
