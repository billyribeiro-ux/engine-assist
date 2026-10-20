<script lang="ts">
	// A diff with + and − characters as well as color (components.md "Outside-change review"),
	// rendered from typed spans; hidden characters are visible escapes (AC-XC-32).
	import type { Diff, RedactedDiff } from '#lib/ipc/types.ts';

	import Text from './Text.svelte';

	let {
		diff = null,
		redacted = null,
		label
	}: { diff?: Diff | null; redacted?: RedactedDiff | null; label: string } = $props();

	type Line = { cls: 'hunk' | 'add' | 'del' | 'ctx'; sign: string; text: string | null; spans: Diff['hunks'][number]['lines'][number]['spans'] | null };

	const lines: Line[] = $derived.by(() => {
		const out: Line[] = [];
		if (diff) {
			for (const h of diff.hunks) {
				out.push({ cls: 'hunk', sign: '', text: `@@ ${diff.path} @@`, spans: null });
				for (const l of h.lines)
					out.push({ cls: l.op, sign: l.op === 'add' ? '+' : l.op === 'del' ? '-' : ' ', text: null, spans: l.spans });
			}
		}
		if (redacted) {
			let last = '';
			for (const e of redacted.entries) {
				const head = e.keyPath.split('.')[0] ?? e.keyPath;
				if (head !== last) out.push({ cls: 'hunk', sign: '', text: `@@ ${head} @@`, spans: null });
				last = head;
				const key = e.keyPath.includes('.') ? `"${e.keyPath.split('.').slice(1).join('.')}": ` : '';
				if (e.before !== null) out.push({ cls: 'del', sign: '-', text: `    ${key}${e.before}`, spans: null });
				if (e.after !== null) out.push({ cls: 'add', sign: '+', text: `    ${key}${e.after}`, spans: null });
			}
		}
		return out;
	});
</script>

<pre class="diff" tabindex="0" aria-label={label}>{#each lines as l, i (i)}<span class={l.cls}
			>{#if l.sign}<span class="sign">{l.sign}</span>{/if}{#if l.spans}<Text
					spans={l.spans}
				/>{:else}<Text text={l.text ?? ''} />{/if}</span
		>{/each}</pre>
