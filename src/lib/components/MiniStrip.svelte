<script lang="ts">
	// Miniature strip (components.md, Today, P2): the 19 sheets at 6 × 14 px, one image with a label.
	import type { SheetStatus } from '#lib/ipc/types.ts';

	let { statuses, label }: { statuses: SheetStatus[]; label: string } = $props();
	const GAPS = new Set([5, 9, 12, 16, 18]);
	const cls = (s: SheetStatus) =>
		s === 'signed'
			? 'sig'
			: s === 'inProgress' || s === 'inspecting' || s === 'waiting'
				? 'cur'
				: s === 'notApplicable'
					? 'na'
					: s === 'needsAnnotation' || s === 'undecided'
						? 'an'
						: '';
</script>

<span class="mini-strip" role="img" aria-label={label}>
	{#each statuses as s, i (i)}<i class={[cls(s), GAPS.has(i) && 'gap']}></i>{/each}
</span>

<style>
	.mini-strip {
		justify-self: start;
		display: inline-flex;
		gap: var(--space-1);
		padding: var(--space-2);
		background: var(--strip-bg);
		vertical-align: middle;
	}
	i {
		width: 6px;
		height: 14px;
		border: 1px solid var(--strip-muted);
		display: block;
	}
	.sig {
		background: var(--strip-line);
		border-color: var(--strip-line);
	}
	.cur {
		background: var(--strip-current-bg);
		border-color: var(--strip-current-bg);
		box-shadow:
			0 0 0 1px var(--strip-bg),
			0 0 0 2px var(--strip-line);
	}
	.na {
		background-image: repeating-linear-gradient(
			135deg,
			color-mix(in srgb, var(--linework) 50%, transparent) 0 1px,
			transparent 1px 3px
		);
	}
	.an {
		border-style: dashed;
	}
	.gap {
		margin-left: 3px;
	}
</style>
