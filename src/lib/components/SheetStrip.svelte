<script lang="ts">
	// The sheet strip (components.md, F-030, AC-F030-06): the one bold element. 19 sheets in six
	// phase groups, each with its number, phase edge and a status mark by shape and color; the open
	// sheet is inverted. One tab stop (roving tabindex): ← → move, Home/End jump, Return opens.
	import type { StageView } from '#lib/ipc/types.ts';
	import { to } from '#lib/nav/routes.ts';
	import { SHEET } from '#lib/nav/status.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import Icon from './Icon.svelte';
	import type { IconName } from './icons.ts';

	let {
		projectId,
		projectName,
		stages,
		open,
		preview = false
	}: {
		projectId: string;
		projectName: string;
		stages: { stage: number; title: string; status: StageView['status']; statusWhy: string | null }[];
		open: number | null;
		/** The wizard's preview strip: not links, nothing opens. */
		preview?: boolean;
	} = $props();

	const PHASES = [
		{ key: 'pre', label: 'Pre-construction', from: 0, to: 4 },
		{ key: 'structure', label: 'Structure', from: 5, to: 8 },
		{ key: 'systems', label: 'Systems', from: 9, to: 11 },
		{ key: 'finish', label: 'Finishes', from: 12, to: 15 },
		{ key: 'co', label: 'Occupancy', from: 16, to: 17 },
		{ key: 'maint', label: 'Ownership', from: 18, to: 18 }
	] as const;
	const TITLES = [
		'Land Survey', 'Meet the Architect', "Architect's Design Studio", 'Permits & Zoning', 'Construction Drawings',
		'Site Prep & Excavation', 'Foundation', 'Framing', 'Roofing', 'Plumbing', 'Electrical', 'HVAC',
		'Insulation & Drywall', 'Siding & Exterior', 'Inspections', 'Paint & Trim', 'Final Walkthrough',
		'Certificate of Occupancy', 'Homeowner Maintenance'
	];

	const MARK: Partial<Record<StageView['status'], IconName>> = {
		ready: 'ring',
		inProgress: 'dot',
		inspecting: 'magnifier',
		waiting: 'magnifier',
		signed: 'stamp',
		undecided: 'question'
	};
	let focusIdx = $state<number | null>(null);
	const tabStop = $derived(focusIdx ?? open ?? stages.findIndex((s) => s.status === 'inProgress' || s.status === 'inspecting') ?? 0);

	function cls(s: (typeof stages)[number]): string[] {
		const c: string[] = [];
		if (s.stage === open) c.push('is-current');
		if (s.status === 'locked') c.push('is-locked');
		if (s.status === 'notApplicable') c.push('is-na');
		if (s.status === 'needsAnnotation' || s.status === 'undecided') c.push('is-annot');
		if (s.status === 'waiting') c.push('is-waiting');
		return c;
	}
	function spoken(s: (typeof stages)[number]): string {
		return `Stage ${s.stage}, ${s.title || TITLES[s.stage] || ''}: ${SHEET[s.status].word}${s.statusWhy ? ` (${s.statusWhy})` : ''}`;
	}
	function onKey(e: KeyboardEvent, idx: number): void {
		const last = stages.length - 1;
		const next =
			e.key === 'ArrowRight' ? Math.min(last, idx + 1) : e.key === 'ArrowLeft' ? Math.max(0, idx - 1) : e.key === 'Home' ? 0 : e.key === 'End' ? last : -1;
		if (next < 0) return;
		e.preventDefault();
		focusIdx = next;
		queueMicrotask(() => document.getElementById(`sheet-${projectId}-${next}`)?.focus());
	}
</script>

<nav class="strip" aria-label="{projectName} drawing set: 19 stages">
	<ol class="strip-groups">
		{#each PHASES as ph (ph.key)}
			<li class="phase-group phase-{ph.key}">
				<ol class="phase-sheets" aria-label="{ph.label}, stages {ph.from} to {ph.to}">
					{#each stages.filter((s) => s.stage >= ph.from && s.stage <= ph.to) as s (s.stage)}
						{@const mark = MARK[s.status]}
						{@const landing = ui.stampStage?.projectId === projectId && ui.stampStage.stage === s.stage}
						<li>
							<svelte:element
								this={preview ? 'span' : 'a'}
								class={['sheet', ...cls(s)]}
								id="sheet-{projectId}-{s.stage}"
								href={preview ? undefined : to.stage(projectId, s.stage)}
								aria-label={spoken(s)}
								title="{s.stage} {s.title || TITLES[s.stage]}: {SHEET[s.status].word}"
								aria-current={s.stage === open ? 'page' : undefined}
								role={preview ? 'img' : undefined}
								tabindex={preview ? undefined : s.stage === tabStop ? 0 : -1}
								onkeydown={preview ? undefined : (e: KeyboardEvent) => onKey(e, s.stage)}
								onfocus={() => (focusIdx = s.stage)}
								><span class="sheet-num" aria-hidden="true">{s.stage}</span>{#if mark}<Icon
										name={mark}
										size="none"
										class="sheet-mark"
										{landing}
									/>{/if}</svelte:element
							>
						</li>
					{/each}
				</ol>
				<span class="phase-dim" aria-hidden="true"><span>{ph.label}</span></span>
			</li>
		{/each}
	</ol>
</nav>

<style>
	.strip {
		position: relative;
		flex: none;
		background-color: var(--strip-bg);
		background-image:
			linear-gradient(var(--strip-grid) 1px, transparent 1px),
			linear-gradient(90deg, var(--strip-grid) 1px, transparent 1px);
		background-size: 12px 12px;
		background-position: -1px -1px;
		color: var(--strip-line);
		padding: 14px var(--space-6) 10px;
		border-bottom: var(--hairline) solid var(--rule);
		container-type: inline-size;
	}
	.strip-groups {
		display: flex;
		gap: 14px;
	}
	.phase-group {
		display: flex;
		flex-direction: column;
		gap: 7px;
		min-width: 0;
	}
	.phase-pre {
		flex: 5 1 0;
		--phase: var(--phase-pre);
	}
	.phase-structure {
		flex: 4 1 0;
		--phase: var(--phase-structure);
	}
	.phase-systems {
		flex: 3 1 0;
		--phase: var(--phase-systems);
	}
	.phase-finish {
		flex: 4 1 0;
		--phase: var(--phase-finish);
	}
	.phase-co {
		flex: 2 1 0;
		--phase: var(--phase-co);
	}
	.phase-maint {
		flex: 1 1 0;
		--phase: var(--phase-maint);
	}
	.phase-sheets {
		display: flex;
		gap: var(--space-2);
	}
	.phase-sheets > li {
		flex: 1 1 0;
		min-width: 0;
	}
	.sheet {
		position: relative;
		display: block;
		height: 50px;
		border: var(--hairline) solid var(--strip-line);
		color: var(--strip-line);
		text-decoration: none;
		font-family: var(--font-drafting);
		font-weight: var(--drafting-weight);
		font-stretch: var(--drafting-stretch);
	}
	a.sheet:hover {
		text-decoration: none;
		background: color-mix(in srgb, var(--linework) 8%, transparent);
	}
	.sheet:focus-visible {
		outline-color: var(--focus-on-strip);
	}
	.sheet::after {
		content: '';
		position: absolute;
		left: 3px;
		right: 3px;
		bottom: 3px;
		height: 3px;
		background: var(--phase);
	}
	.sheet-num {
		position: absolute;
		left: 6px;
		bottom: 9px;
		font-size: var(--text-18);
		line-height: 1;
		letter-spacing: 0.01em;
	}
	.sheet :global(.sheet-mark) {
		position: absolute;
		top: 5px;
		right: 5px;
		width: 14px;
		height: 14px;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.4;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.sheet :global(.sheet-mark circle[fill='currentColor']) {
		fill: currentColor;
	}
	.sheet.is-locked {
		border-color: var(--strip-muted);
		color: var(--strip-muted);
	}
	.sheet.is-na {
		border-color: var(--strip-muted);
		color: var(--strip-muted);
		background-image: repeating-linear-gradient(
			135deg,
			color-mix(in srgb, var(--linework) 34%, transparent) 0 1px,
			transparent 1px 6px
		);
	}
	.sheet.is-annot {
		border-style: dashed;
		border-color: var(--strip-muted);
		color: var(--strip-muted);
	}
	.sheet.is-current {
		background: var(--strip-current-bg);
		color: var(--strip-current-ink);
		border-color: var(--strip-current-bg);
		border-style: solid;
	}
	.sheet.is-current::before {
		content: '';
		position: absolute;
		left: 50%;
		bottom: -9px;
		width: 8px;
		height: 8px;
		margin-left: -4px;
		background: var(--strip-current-bg);
		transform: rotate(45deg);
	}
	.sheet.is-waiting :global(.sheet-mark) {
		color: var(--strip-attention);
	}
	.sheet :global(.stamp) {
		transform: rotate(-14deg);
		transform-origin: 50% 50%;
	}
	.sheet :global(.stamp.is-landing) {
		animation: stamp-in var(--dur-stamp) var(--ease-stamp) both;
		animation-delay: 300ms;
	}
	@keyframes stamp-in {
		0% {
			transform: rotate(-14deg) scale(1.9);
			opacity: 0;
		}
		60% {
			transform: rotate(-14deg) scale(0.9);
			opacity: 1;
		}
		100% {
			transform: rotate(-14deg) scale(1);
			opacity: 1;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.sheet :global(.stamp.is-landing) {
			animation: none;
		}
	}
	@media (prefers-contrast: more) {
		.sheet.is-na,
		.sheet.is-annot {
			background-image: none;
			border-style: solid;
		}
	}
	.phase-dim {
		position: relative;
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: var(--text-11);
		color: var(--strip-muted);
		line-height: 1;
		white-space: nowrap;
		overflow: hidden;
		border-left: 1px solid var(--phase);
		border-right: 1px solid var(--phase);
		height: 9px;
	}
	.phase-dim::before,
	.phase-dim::after {
		content: '';
		flex: 1;
		height: 1px;
		background: var(--phase);
		min-width: 6px;
	}
	.phase-dim span {
		padding: 0 var(--space-1);
	}
	.phase-maint .phase-dim {
		overflow: visible;
	}
	.phase-maint .phase-dim::before,
	.phase-maint .phase-dim::after {
		min-width: 2px;
	}
	@container (max-width: 920px) {
		.sheet {
			height: 44px;
		}
		.sheet-num {
			font-size: var(--text-15);
			left: 4px;
			bottom: 8px;
		}
		.sheet :global(.sheet-mark) {
			width: 12px;
			height: 12px;
			top: 3px;
			right: 3px;
		}
		.strip-groups {
			gap: var(--space-3);
		}
		.phase-sheets {
			gap: 3px;
		}
	}
</style>
