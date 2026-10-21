<script lang="ts">
	// The open project's menu in the title bar (AC-F017-10, AC-F014-11): pin to a ⌘1–⌘9 slot, Open
	// terminal here, Remove. A hand-built menu button: ↑↓ move, Home/End jump, Esc closes and returns
	// focus, a letter-free design so nothing here needs a shortcut. Remove only asks; Rust raises the
	// native confirmation (SR-24a).
	import { tick } from 'svelte';

	import { openTerminalHere, removeProject, togglePin } from '#lib/actions/project.ts';
	import type { ProjectSummary } from '#lib/ipc/types.ts';

	import Icon from './Icon.svelte';
	import type { IconName } from './icons.ts';

	let { project }: { project: ProjectSummary } = $props();

	let open = $state(false);
	let button = $state<HTMLButtonElement>();
	let menu = $state<HTMLElement>();
	const uid = `pm-${Math.random().toString(36).slice(2, 8)}`;

	interface Entry {
		id: string;
		label: string;
		icon: IconName;
		run: () => Promise<void>;
		danger?: boolean;
	}
	const entries: Entry[] = $derived([
		{
			id: 'pin',
			label: project.pinned ? `Unpin from ⌘${project.pinned}` : 'Pin to a ⌘ number',
			icon: 'link',
			run: () => togglePin(project.id)
		},
		{
			id: 'terminal',
			label: 'Open terminal here',
			icon: 'terminal',
			run: () => openTerminalHere(project.id)
		},
		{
			id: 'remove',
			label: 'Remove project…',
			icon: 'cross',
			run: () => removeProject(project.id),
			danger: true
		}
	]);

	function items(): HTMLElement[] {
		return menu ? [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')] : [];
	}
	async function show(): Promise<void> {
		open = true;
		await tick();
		items()[0]?.focus();
	}
	function hide(back = true): void {
		open = false;
		if (back) button?.focus();
	}
	function onButtonKey(e: KeyboardEvent): void {
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			e.preventDefault();
			void show();
		}
	}
	function onMenuKey(e: KeyboardEvent): void {
		const els = items();
		const i = els.findIndex((el) => el === document.activeElement);
		const to = (n: number): void => {
			e.preventDefault();
			els[(n + els.length) % els.length]?.focus();
		};
		if (e.key === 'ArrowDown') to(i + 1);
		else if (e.key === 'ArrowUp') to(i - 1);
		else if (e.key === 'Home') to(0);
		else if (e.key === 'End') to(els.length - 1);
		else if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			hide();
		} else if (e.key === 'Tab') hide(false);
	}
	async function choose(e: Entry): Promise<void> {
		hide();
		await e.run();
	}
</script>

<div class="pmenu">
	<button
		class="btn ghost"
		type="button"
		bind:this={button}
		aria-haspopup="menu"
		aria-expanded={open}
		aria-controls={open ? uid : undefined}
		onclick={() => (open ? hide() : show())}
		onkeydown={onButtonKey}>Project<Icon name="chev" size="glyph" /></button
	>
	{#if open}
		<div class="pmenu-scrim" aria-hidden="true" onclick={() => hide(false)}></div>
		<div
			class="pmenu-list"
			id={uid}
			role="menu"
			aria-label="{project.name} actions"
			tabindex="-1"
			bind:this={menu}
			onkeydown={onMenuKey}
		>
			{#each entries as e (e.id)}
				<button
					class={['pmenu-item', e.danger && 'danger']}
					type="button"
					role="menuitem"
					tabindex="-1"
					onclick={() => choose(e)}><Icon name={e.icon} />{e.label}</button
				>
			{/each}
		</div>
	{/if}
</div>

<style>
	.pmenu {
		position: relative;
		display: inline-flex;
	}
	.pmenu-scrim {
		position: fixed;
		inset: 0;
		z-index: 19;
	}
	.pmenu-list {
		position: absolute;
		top: calc(100% + var(--space-2));
		right: 0;
		z-index: 20;
		min-width: 220px;
		display: grid;
		padding: var(--space-2);
		background: var(--sheet-raised);
		border-radius: var(--radius-overlay);
		box-shadow: var(--shadow-overlay);
	}
	.pmenu-item {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: 6px var(--space-3);
		border: 0;
		border-radius: var(--radius-control);
		background: transparent;
		color: var(--ink);
		font-size: var(--text-13);
		text-align: left;
		cursor: default;
	}
	.pmenu-item:hover,
	.pmenu-item:focus-visible {
		background: var(--selected);
	}
	.pmenu-item.danger {
		color: var(--fail);
	}
	.pmenu-item :global(.icon) {
		color: var(--ink-muted);
	}
</style>
