<script lang="ts">
	// Sidebar (components.md "Layout"): Today, projects (the open one expands to its stage list, the
	// P1 stage navigation), and the footer: Run log, Template health, Doctor, Settings. Every status
	// is a glyph plus words (AC-XC-25); the Doctor dot shows its last level (AC-F019-07).
	import { page } from '$app/state';
	import type { InboxItem, ProjectSummary, StageView } from '#lib/ipc/types.ts';
	import { to } from '#lib/nav/routes.ts';
	import { DOCTOR, SHEET } from '#lib/nav/status.ts';
	import { current } from '#lib/stores/project.svelte.ts';
	import { resource } from '#lib/stores/resource.svelte.ts';

	import Icon from './Icon.svelte';

	const projects = resource(
		() => null,
		(c) => c.projectsList({}),
		() => ['projects']
	);
	const today = resource(
		() => null,
		(c) => c.todayGet({}),
		() => ['today']
	);
	const boot = resource(
		() => null,
		(c) => c.appBootstrap({}),
		() => ['doctor', 'projects']
	);
	let doctorLevel = $state<'ok' | 'warning' | 'failing' | 'unknown'>('unknown');
	$effect(() => {
		if (boot.value) doctorLevel = boot.value.doctor;
	});

	const path = $derived(page.url.pathname);
	const inbox: InboxItem[] = $derived(today.value?.inbox ?? []);
	const openId = $derived(page.params.projectId ?? null);

	const FAIL = new Set(['packetInvalid', 'gateFailed', 'tamper']);
	function badge(p: ProjectSummary): 'fail' | 'attention' | null {
		const mine = inbox.filter((i) => i.project === p.id);
		if (p.readOnly || mine.some((i) => FAIL.has(i.kind))) return 'fail';
		if (mine.some((i) => i.kind === 'gateSignable' || i.kind === 'packetLanded')) return 'attention';
		return null;
	}

	interface Row {
		key: string;
		num: string;
		name: string;
		stage: StageView;
		spoken: string;
	}
	/** Consecutive signed stages, and the stages beyond 5 the blueprint doesn't annotate, fold into one row. */
	const rows: Row[] = $derived.by(() => {
		const v = current.project;
		if (!v || v.summary.id !== openId) return [];
		const out: Row[] = [];
		const cls = (s: StageView) =>
			s.status === 'signed'
				? 'signed'
				: s.stage > 5 && ['needsAnnotation', 'undecided', 'notApplicable'].includes(s.status)
					? 'annot'
					: `one-${s.stage}`;
		let i = 0;
		while (i < v.stages.length) {
			const s = v.stages[i];
			if (!s) break;
			let j = i;
			while (j + 1 < v.stages.length && cls(v.stages[j + 1] ?? s) === cls(s) && !cls(s).startsWith('one'))
				j += 1;
			const last = v.stages[j] ?? s;
			if (j > i) {
				const signed = cls(s) === 'signed';
				out.push({
					key: `${s.stage}-${last.stage}`,
					num: `${s.stage}–${last.stage}`,
					name: signed ? 'Signed' : 'Need annotation',
					stage: signed ? s : { ...s, status: 'needsAnnotation' },
					spoken: signed ? 'signed' : 'need annotation'
				});
			} else out.push({ key: `${s.stage}`, num: `${s.stage}`, name: s.title, stage: s, spoken: SHEET[s.status].word });
			i = j + 1;
		}
		return out;
	});
	const currentStage = $derived(page.params.stage === undefined ? null : Number(page.params.stage));
	const doctorLook = $derived(DOCTOR[doctorLevel]);
</script>

<nav class="sidebar" aria-label="Foreman">
	<div class="traffic" aria-hidden="true"></div>
	<div class="side-scroll">
		<a class="side-item" href={to.today()} aria-current={path === '/' ? 'page' : undefined}
			><Icon name="tray" /><span class="label">Today</span>
			{#if inbox.length}<span class="badge" aria-label="{inbox.length} items waiting on you"
					>{inbox.length}</span
				>{/if}</a
		>
		<div class="side-head" id="side-projects">Projects</div>
		{#if projects.value && projects.value.length === 0}
			<p class="side-empty">No projects yet</p>
		{/if}
		<ul aria-labelledby="side-projects">
			{#each projects.value ?? [] as p (p.id)}
				{@const b = badge(p)}
				{@const isOpen = p.id === openId}
				<li>
					<a
						class="side-item"
						href={to.project(p.id)}
						aria-current={isOpen ? 'page' : undefined}
						title={p.pinned ? `${p.name} (⌘${p.pinned})` : p.name}
						><Icon name="folder" /><span class="label">{p.name}</span>
						{#if !isOpen && b === 'fail'}
							<span class="badge" aria-label={p.readOnly ? 'Read-only' : '1 problem'}
								><Icon name="cross" size="glyph" tone="fail" /></span
							>
						{:else if !isOpen && b === 'attention'}
							<span class="badge" aria-label="Packet waiting for your signature"
								><span class="dot attention"></span></span
							>
						{:else if p.currentStage !== null}
							<span class="meta">Stage {p.currentStage}</span>
						{/if}</a
					>
					{#if isOpen && rows.length}
						<ol class="side-stages" aria-label="{p.name} stages">
							{#each rows as r (r.key)}
								{@const look = SHEET[r.stage.status]}
								<li>
									<a
										class="side-stage"
										href={to.stage(p.id, r.stage.stage)}
										aria-current={currentStage === r.stage.stage ? 'step' : undefined}
										aria-label="Stage {r.num}, {r.name}: {r.spoken}"
										><Icon name={look.icon} size="glyph" tone={look.tone} /><span class="num"
											>{r.num}</span
										><span>{r.name}</span></a
									>
								</li>
							{/each}
						</ol>
					{/if}
				</li>
			{/each}
		</ul>
	</div>
	<div class="side-foot">
		<a class="side-item" href={to.runLog()} aria-current={path === '/run-log' ? 'page' : undefined}
			><Icon name="log" /><span class="label">Run log</span></a
		>
		<a
			class="side-item"
			href={to.templateHealth()}
			aria-current={path === '/template-health' ? 'page' : undefined}
			><Icon name="file" /><span class="label">Template health</span></a
		>
		<a class="side-item" href={to.doctor()} aria-current={path === '/doctor' ? 'page' : undefined}
			><Icon name="pulse" /><span class="label">Doctor</span>
			{#if doctorLevel === 'failing'}
				<span class="badge" aria-label="Doctor: {doctorLook.word}"
					><Icon name="cross" size="glyph" tone="fail" /></span
				>
			{:else if doctorLevel === 'warning'}
				<span class="badge" aria-label="Doctor: warnings"><span class="dot attention"></span></span>
			{:else if doctorLevel === 'ok'}
				<span class="badge" aria-label="Doctor: all checks pass"><span class="dot pass"></span></span>
			{/if}</a
		>
		<a class="side-item" href={to.settings()} aria-current={path === '/settings' ? 'page' : undefined}
			><Icon name="sliders" /><span class="label">Settings</span><span class="meta">⌘,</span></a
		>
	</div>
</nav>

<style>
	.side-empty {
		font-size: var(--text-12);
		color: var(--ink-muted);
		padding: var(--space-1) var(--space-3);
	}
</style>
