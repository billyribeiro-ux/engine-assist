<script lang="ts">
	import { page } from '$app/state';

	import Gate from '#lib/screens/Gate.svelte';
	import { current } from '#lib/stores/project.svelte.ts';
</script>

{#if current.project}
	<Gate project={current.project} stageNo={Number(page.params.stage)} />
{/if}
