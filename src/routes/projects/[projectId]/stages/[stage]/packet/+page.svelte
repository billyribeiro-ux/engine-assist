<script lang="ts">
	import { page } from '$app/state';

	import Packet from '#lib/screens/Packet.svelte';
	import { current } from '#lib/stores/project.svelte.ts';
</script>

{#if current.project}
	<Packet project={current.project} stageNo={Number(page.params.stage)} />
{/if}
