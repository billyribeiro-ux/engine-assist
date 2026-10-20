// Foreman-written sentences for typed results (A.9 writing rules). The UI never parses Rust's
// `message`; it branches on `code` and `detail.kind` (ADR-004 §5).
import { IPC_ERROR_TEXT, type IpcFailure } from '#lib/ipc/errors.ts';
import type { Blocker, ConfirmReason, Model, Mode, Start } from '#lib/ipc/types.ts';

export const MODEL_NAME: Record<Model, string> = {
	fable: 'Fable 5.1',
	opus: 'Opus 5.5',
	sonnet: 'Sonnet 5.5',
	haiku: 'Haiku 5.5',
	opusplan: 'Opus plan'
};
export const MODE_NAME: Record<Mode, string> = {
	plan: 'Plan',
	default: 'Manual',
	acceptEdits: 'Accept edits',
	auto: 'Auto',
	dontAsk: "Don't ask"
};
const startText = (s: Start) =>
	`${MODEL_NAME[s.model]}, ${s.effort}${s.mode ? `, ${MODE_NAME[s.mode]}` : ''}`;

export function blockerText(b: Blocker): string {
	switch (b.kind) {
		case 'notRunnable':
			return b.text;
		case 'readOnly':
			return `This project is open read-only: ${b.detail}`;
		case 'claudeMissing':
			return `Claude Code isn't usable at ${b.path ?? 'the configured path'} (${
				{
					relative: 'a relative path',
					missing: 'missing',
					notExecutable: 'not executable',
					inProject: 'inside a project',
					symlinkIntoProject: 'a symlink into a project',
					worldWritable: 'in a folder anyone can write to'
				}[b.why]
			}). Fix it in Settings or Doctor.`;
		case 'versionBelowFloor':
			return `Claude Code ${b.found} found. ${b.requirements[0]?.feature ?? 'This step'} needs ${b.required} or later. Run claude update, then check again.`;
		case 'contractFailed':
			return `Claude Code ${b.version} no longer accepts ${b.feature}. Doctor has the details.`;
		case 'configChanged':
			return `Claude Code configuration in ${b.cwd === 'spec' ? 'spec/' : 'the project'} changed. Review it and acknowledge it first.`;
		case 'authKeyBlocked':
			return `${b.key} in ${b.file} would move billing off your plan. Remove it, then check again.`;
		case 'configTooLarge':
			return `${b.file} is too large to check (${b.bytes} bytes).`;
		case 'projectEnvUnapproved':
			return `${b.file} sets environment variables. Approve them first.`;
		case 'authFailed':
			return b.why.kind === 'wrongMethod'
				? `Claude Code is signed in with ${b.why.method}. Sign in with your claude.ai plan.`
				: "Claude Code isn't signed in with your plan. Run claude auth login, then check again.";
		case 'transcriptMissing':
			return 'This conversation no longer exists (Claude Code removes transcripts after cleanupPeriodDays).';
		case 'signedPacketMissing':
			return `The signed Stage ${b.stage} packet is missing. Restore it first.`;
		case 'packetUnsigned':
			return `Sign Stage ${b.stage} first.`;
		case 'runInputMissing':
			return `Fill ${b.name} to run this step.`;
		case 'unresolvedToken':
			return `Fill ${b.token} to run this step.`;
		case 'promptOptionLike':
			return `The prompt starts with "${b.first}", which Claude Code would read as an option.`;
		case 'promptControlChar':
			return `The prompt has a control character (U+${b.codepoint.toString(16).toUpperCase().padStart(4, '0')}) at ${b.offset}. Remove it from the input.`;
		case 'runSettingsPathUnsafe':
			return `The run settings path ${b.path} can't be used safely.`;
		case 'cwdNotDirectory':
			return `The step's folder (${b.cwd === 'spec' ? 'spec/' : 'project'}) doesn't exist.`;
	}
}

export function confirmText(c: ConfirmReason): string {
	switch (c.kind) {
		case 'overlayStep':
			return 'This step was added for this project.';
		case 'modeOrModelChanged':
			return `Runs with ${startText(c.to)} instead of the template's ${startText(c.from)}.`;
		case 'firstLaunchInProject':
			return 'This is the first launch in this project.';
		case 'autoOrDontAsk':
			return `Runs in ${MODE_NAME[c.mode]} mode.`;
		case 'claudeVersionChanged':
			return `Claude Code updated ${c.from} → ${c.to}.`;
	}
}

/** One sentence for a failed command, by code and detail (never Rust's message text). */
export function failureText(e: IpcFailure): string {
	const d = e.detail;
	switch (d?.kind) {
		case 'notAllowed':
			return RULES[d.rule] ?? IPC_ERROR_TEXT.notAllowed;
		case 'clipboard':
			return d.why === 'empty'
				? 'The clipboard is empty. Copy the text first.'
				: d.why === 'tooLarge'
					? 'The clipboard has more than 1 MB of text.'
					: "The clipboard doesn't hold text.";
		case 'conflict':
			return `${d.path} changed outside Foreman.${d.written.length ? ` Already written: ${d.written.join(', ')}.` : ''} Review it, then try again.`;
		case 'policyViolation':
			return `Foreman doesn't write ${d.path} (${d.rule}).`;
		case 'readOnly':
			return `This project is open read-only: ${d.detail}`;
		case 'stale':
			return IPC_ERROR_TEXT.stale;
		case 'toolMissing':
			return `${d.tool === 'git' ? 'git' : 'Claude Code'} isn't found${d.path ? ` at ${d.path}` : ''}. Doctor shows how to fix it.`;
		case 'toolFailed':
			return `${d.tool} failed${d.exit === null ? '' : ` with exit ${d.exit}`}${d.timedOut ? ' (timed out)' : ''}.`;
		case 'unsupported':
			return `${IPC_ERROR_TEXT.unsupported} (${d.why})`;
		case 'invalid':
			return `${IPC_ERROR_TEXT.invalid} (${d.field})`;
		default:
			return IPC_ERROR_TEXT[e.code];
	}
}

const RULES: Record<string, string> = {
	'core-step-skip': 'Core steps can never be skipped.',
	'conditional-step-skip':
		'A conditional step applies or not by its condition; it is never skipped by hand.',
	'reason-required': 'Write a one-line reason first.',
	'condition-locked-after-stage-2':
		'Change requires a Change Request (v1.1): conditions are frozen once Stage 2 is signed. See docs/templates/change-request.md.',
	'late-condition-locked-after-stage-4':
		'Change requires a Change Request (v1.1): late conditions are frozen once Stage 4 is signed.',
	'unresolved-tokens': 'Fill the highlighted inputs first.',
	'prompt-control-char': 'The prompt has a control character. Remove it from the input.',
	'cli-finishes-by-exit': 'CLI steps finish when Claude Code exits, not by marking them done.',
	'pty-not-live': 'That session has ended.',
	'needs-rearm': 'This step continues a live session. Run re-arms it.',
	'no-editor-set': 'Set an editor in Settings to open files.',
	'not-signable': 'Sign is locked until the gate passes and the packet is valid.',
	'note-too-short': 'Write at least 15 characters on how you checked.',
	'note-repeated': "That note repeats another item's. Say how you checked this one.",
	'wrong-stage-packet': 'This packet is for another stage than this step expects.',
	'two-packets': 'Paste one packet at a time.',
	'no-packet': "The clipboard doesn't hold a handoff packet.",
	'runs-live': 'End the running sessions first.',
	'chain-live': 'That session is still running.',
	'repin-after-stage-1-signed': "The blueprint pin can't change after Stage 1 is signed.",
	'fresh-not-continuing': "This step doesn't continue a session.",
	'no-stage-4-packet': 'The build package needs a valid Stage 4 packet. Run 4.8 first.',
	'process-still-alive': 'Claude is still running. End it yourself, then press Continue.',
	'not-runnable': "This step can't run now.",
	'item-not-failed': 'Only a failed item can be fixed.'
};
