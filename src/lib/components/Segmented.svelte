<script lang="ts" generics="V extends string">
	// Segmented control as a radiogroup (components.md "Setup form fields"): one tab stop,
	// ← → move and select, Space/Return select. Used for tri-state conditions, Done / Not applicable,
	// the theme and filters.
	let {
		options,
		value,
		label,
		labelledby = null,
		disabled = false,
		onchange
	}: {
		options: { value: V; label: string }[];
		value: V | null;
		label?: string;
		labelledby?: string | null;
		disabled?: boolean;
		onchange: (v: V) => void;
	} = $props();

	let group = $state<HTMLDivElement>();
	const focusIdx = $derived(
		Math.max(
			0,
			options.findIndex((o) => o.value === value)
		)
	);

	function onKey(e: KeyboardEvent, i: number): void {
		const n = options.length;
		const j =
			e.key === 'ArrowRight' || e.key === 'ArrowDown'
				? (i + 1) % n
				: e.key === 'ArrowLeft' || e.key === 'ArrowUp'
					? (i - 1 + n) % n
					: -1;
		if (j < 0) return;
		e.preventDefault();
		const o = options[j];
		if (!o || disabled) return;
		onchange(o.value);
		queueMicrotask(() => group?.querySelectorAll<HTMLElement>('[role="radio"]')[j]?.focus());
	}
</script>

<div
	class="seg"
	role="radiogroup"
	aria-label={labelledby ? undefined : label}
	aria-labelledby={labelledby ?? undefined}
	aria-disabled={disabled ? 'true' : undefined}
	bind:this={group}
>
	{#each options as o, i (o.value)}
		<button
			type="button"
			role="radio"
			aria-checked={o.value === value}
			tabindex={i === focusIdx ? 0 : -1}
			{disabled}
			onclick={() => onchange(o.value)}
			onkeydown={(e) => onKey(e, i)}>{o.label}</button
		>
	{/each}
</div>
