<script lang="ts">
	import OutsideChange from '#lib/screens/OutsideChange.svelte';
	import { current } from '#lib/stores/project.svelte.ts';
</script>

{#if current.project}
	<OutsideChange project={current.project} />
{/if}
