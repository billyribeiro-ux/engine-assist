<script lang="ts">
	// Chat setup chips (F-012, AC-F012-01, mockup workspace-chat.html): checkable reminders from the
	// step's RUN WITH line (the session chip in bold, the model, effort, web search, Research).
	// Foreman can't set any of these in claude.ai; the checks live only in this view.
	import type { ChatChips } from '#lib/ipc/types.ts';

	import Icon from './Icon.svelte';

	let { chips }: { chips: ChatChips } = $props();
	let on = $state<Record<string, boolean>>({});
</script>

<div class="chips" role="group" aria-label="Chat setup: check each before you send">
	{#each chips.chips as ch (ch.id)}
		<button
			type="button"
			class={['chip', ch.strong && 'strong']}
			role="checkbox"
			aria-checked={on[ch.id] ?? false}
			onclick={() => (on[ch.id] = !(on[ch.id] ?? false))}
			><span class={['box', on[ch.id] && 'on']} aria-hidden="true"
				>{#if on[ch.id]}<Icon name="check" size="none" />{/if}</span
			>{ch.label}</button
		>
	{/each}
	<span class="chips-note">Reminders only; Foreman can't set these in claude.ai.</span>
</div>
