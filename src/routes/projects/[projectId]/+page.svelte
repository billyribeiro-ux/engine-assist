<script lang="ts">
	// A project opens at its current stage.
	import { goto } from '$app/navigation';

	import { current } from '#lib/stores/project.svelte.ts';
	import { to } from '#lib/nav/routes.ts';

	$effect(() => {
		const p = current.project;
		if (p) void goto(to.stage(p.summary.id, p.summary.currentStage ?? 1), { replace: true });
	});
</script>
