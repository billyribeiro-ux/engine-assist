<script lang="ts">
	import Assembler from '#lib/screens/Assembler.svelte';
	import { current } from '#lib/stores/project.svelte.ts';
</script>

{#if current.project}
	<Assembler project={current.project} />
{/if}
