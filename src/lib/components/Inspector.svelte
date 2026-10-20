<script lang="ts">
	// Inspector (components.md, F-030 P2, AC-F030-08): Packet, Gate and Lessons for the open stage.
	// ⌘⌥I collapses it; below 1360 px it is hidden and ⌘⌥I brings it back as an overlay. Tabs are a
	// tablist (← → move); empty states invite the next action.
	import type { Snippet } from 'svelte';

	import type { Manifest, ProjectView, StageView, StepSummary } from '#lib/ipc/types.ts';
	import { to } from '#lib/nav/routes.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	import Icon from './Icon.svelte';
	import KnowledgeManifest from './KnowledgeManifest.svelte';

	let {
		project,
		stage,
		step = null,
		initial = 'gate',
		label = 'Inspector',
		packetPane = null,
		gatePane = null
	}: {
		project: ProjectView;
		stage: StageView;
		step?: StepSummary | null;
		initial?: 'packet' | 'gate' | 'lessons';
		label?: string;
		packetPane?: Snippet | null;
		gatePane?: Snippet | null;
	} = $props();

	type TabId = 'packet' | 'gate' | 'lessons';
	let chosen = $state<TabId | null>(null);
	const tab: TabId = $derived(chosen ?? initial);
	const TABS: { id: TabId; label: string }[] = [
		{ id: 'packet', label: 'Packet' },
		{ id: 'gate', label: 'Gate' },
		{ id: 'lessons', label: 'Lessons' }
	];
	const pid = $derived(project.summary.id);
	const gate = resource(
		() =>
			tab === 'gate' && !gatePane
				? { projectId: pid, stage: stage.stage, rev: project.summary.rev }
				: null,
		(c, a) => (a ? c.gateView({ projectId: a.projectId, stage: a.stage }) : Promise.resolve(null)),
		(a) => (a ? [`project:${a.projectId}`] : [])
	);
	const lessons = resource(
		() => ({ projectId: pid, stage: stage.stage, rev: project.summary.rev }),
		(c, a) => c.lessonsStatus({ projectId: a.projectId, stage: a.stage }),
		(a) => [`project:${a.projectId}`]
	);
	const manifest = resource(
		() => (tab === 'packet' && step?.surface === 'chat' ? pid : null),
		(c, id) => (id ? c.manifestView({ projectId: id }) : Promise.resolve(null)),
		(id) => (id ? [`project:${id}`] : [])
	);
	const packetWriter = $derived(
		stage.steps.find(
			(s) => (s.specLine?.note?.includes('packet') ?? false) || s.stateText.includes('packet')
		)?.number ??
			stage.steps.filter((s) => s.kind !== 'lessons' && s.kind !== 'gate').at(-1)?.number ??
			'the last step'
	);

	function onKey(e: KeyboardEvent, i: number): void {
		const n = TABS.length;
		const j = e.key === 'ArrowRight' ? (i + 1) % n : e.key === 'ArrowLeft' ? (i - 1 + n) % n : -1;
		if (j < 0) return;
		e.preventDefault();
		chosen = TABS[j]?.id ?? 'gate';
		queueMicrotask(() => document.getElementById(`insp-t-${chosen ?? ''}`)?.focus());
	}
	const lessonsEmpty = $derived(
		`No lessons for Stage ${stage.stage} yet. F.2 writes them to docs/lessons.md ${stage.steps.some((s) => s.surface === 'cli') ? 'in the council session' : 'in the chat'} after you attest the items.`
	);
</script>

<aside
	class={['inspector', !ui.inspectorOpen && 'closed', ui.inspectorOverlay && 'forced']}
	aria-label={label}
>
	<div class="insp-tabs" role="tablist" aria-label="Inspector views">
		{#each TABS as t, i (t.id)}
			<button
				role="tab"
				type="button"
				id="insp-t-{t.id}"
				aria-selected={tab === t.id}
				aria-controls="insp-p-{t.id}"
				tabindex={tab === t.id ? 0 : -1}
				onclick={() => (chosen = t.id)}
				onkeydown={(e) => onKey(e, i)}>{t.label}</button
			>
		{/each}
	</div>
	<div class="insp-body" role="tabpanel" id="insp-p-{tab}" aria-labelledby="insp-t-{tab}">
		{#if tab === 'packet'}
			{#if packetPane}
				{@render packetPane()}
			{:else if stage.packet?.exists}
				<div>
					<h2 class="insp-title">Stage {stage.stage} packet</h2>
					<p class="insp-sub">
						<code>{stage.packet.path}</code>
						{stage.packet.signed
							? 'is signed.'
							: stage.packet.problems
								? `has ${stage.packet.problems} problems.`
								: 'is valid and waits for your signature.'}
					</p>
				</div>
				<div class="actions">
					<a class="btn" href={to.packet(pid, stage.stage)}><Icon name="file" />Open the packet</a>
				</div>
			{:else}
				<div class="empty">
					<strong>No Stage {stage.stage} packet yet</strong>
					{#if step?.surface === 'chat'}
						<span
							>Send {packetWriter} in the chat, say "approved", copy the packet Claude prints, then press
							⌘⇧V on
							{packetWriter}. Foreman validates it before writing anything.</span
						>
					{:else}
						<span
							>{packetWriter} writes docs/handoff/stage-{String(stage.stage).padStart(2, '0')}.md.
							Foreman checks it when it lands.</span
						>
					{/if}
				</div>
			{/if}
			{#if step?.surface === 'chat' && manifest.value}
				<KnowledgeManifest
					projectId={pid}
					manifest={manifest.value}
					onchange={(m: Manifest) => manifest.set(m)}
				/>
			{:else if step?.surface === 'chat'}
				<div>
					<h2 class="insp-title">Knowledge for this chat</h2>
					<p class="insp-sub">
						Nothing to upload yet. Signed packets appear here for the claude.ai Project.
					</p>
				</div>
			{/if}
		{:else if tab === 'gate'}
			{#if gatePane}
				{@render gatePane()}
			{:else}
				<div>
					<h2 class="insp-title">Stage {stage.stage} inspection</h2>
					<p class="insp-sub">
						You attest each item after {packetWriter} writes the packet. Each needs a note, or a reason
						it doesn't apply.
					</p>
				</div>
				{#if gate.value}
					<ol class="insp-list">
						{#each gate.value.items as it (it.id)}
							<li>
								<Icon
									name={it.status === 'pass'
										? 'check'
										: it.status === 'fail'
											? 'cross'
											: it.status === 'notApplicable'
												? 'hatch'
												: 'ring'}
									size="glyph"
									tone={it.status === 'pass' ? 'pass' : it.status === 'fail' ? 'fail' : 'muted'}
								/>
								<span
									>{it.text}<span class="kind"
										>{it.waitsFor ??
											(it.status === 'pass'
												? 'Passed'
												: it.status === 'fail'
													? 'Failed'
													: 'Attest')}</span
									></span
								>
							</li>
						{/each}
					</ol>
					<div class="actions">
						{#if stage.packet?.exists}
							<a class="btn" href={to.gate(pid, stage.stage)}
								>Close out Stage {stage.stage}<kbd>⌘⇧G</kbd></a
							>
						{:else}
							<button class="btn" type="button" aria-disabled="true" aria-describedby="why-close"
								>Close out Stage {stage.stage}<kbd>⌘⇧G</kbd></button
							>
						{/if}
					</div>
					{#if !stage.packet?.exists}
						<p class="reason" id="why-close">
							Available after {packetWriter} writes docs/handoff/stage-{String(
								stage.stage
							).padStart(2, '0')}.md.
						</p>
					{/if}
				{/if}
			{/if}
			{#if lessons.value?.entries.length === 0}
				<div class="empty" aria-label="Lessons">
					<strong>No lessons for Stage {stage.stage} yet</strong>
					<span>{lessonsEmpty.replace(`No lessons for Stage ${stage.stage} yet. `, '')}</span>
				</div>
			{/if}
		{:else}
			<div>
				<h2 class="insp-title">Lessons</h2>
				<p class="insp-sub">
					docs/lessons.md, entries headed “Stage {stage.stage}”. Next ID {lessons.value?.nextId ??
						''}.
				</p>
			</div>
			{#if lessons.value?.entries.length}
				<ul class="insp-list">
					{#each lessons.value.entries as l (l.id)}
						<li>
							<Icon name="file" size="glyph" tone="muted" /><span><b>{l.id}</b> {l.title}</span>
						</li>
					{/each}
				</ul>
			{:else}
				<div class="empty">
					<strong>No lessons for Stage {stage.stage} yet</strong>
					<span>{lessonsEmpty.replace(`No lessons for Stage ${stage.stage} yet. `, '')}</span>
				</div>
			{/if}
		{/if}
	</div>
</aside>

<style>
	.inspector.closed {
		display: none;
	}
	@media (max-width: 1359px) {
		.inspector.forced {
			display: block;
			position: fixed;
			right: 0;
			top: var(--titlebar-height);
			bottom: 0;
			width: var(--inspector-width-compact);
			z-index: 15;
			box-shadow: var(--shadow-overlay);
		}
	}
</style>
