<script lang="ts">
	import '../app.css';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<main>
	{@render children()}
</main>

<style>
	main {
		max-width: 60rem;
		margin: 0 auto;
		padding: 2.5rem 1.5rem;
	}
</style>
