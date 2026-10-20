<script lang="ts">
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<h1>Today</h1>

{#if data.error}
	<p role="alert">Foreman couldn't start: {data.error.message}</p>
{:else if data.boot.firstRun}
	<p>Welcome to Foreman. First-run setup arrives in M2.</p>
	<p class="muted">Projects: {data.boot.projects.length} · Doctor: {data.boot.doctor}</p>
{:else}
	<p>{data.boot.projects.length} projects.</p>
{/if}

<style>
	.muted {
		color: var(--fm-muted);
	}
</style>
