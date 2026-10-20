// Shape, color and words for every status (AC-XC-25: never color alone).
import type { IconName } from '#lib/components/icons.ts';
import type { SheetStatus, StepState, RunStatus, DoctorLevel } from '#lib/ipc/types.ts';

export type Tone = 'pass' | 'fail' | 'attention' | 'muted' | 'accent';
export interface Look {
	icon: IconName;
	tone: Tone;
	word: string;
}

export const SHEET: Record<SheetStatus, Look> = {
	locked: { icon: 'lock', tone: 'muted', word: 'locked' },
	ready: { icon: 'ring', tone: 'muted', word: 'ready' },
	inProgress: { icon: 'dot', tone: 'accent', word: 'in progress' },
	inspecting: { icon: 'magnifier', tone: 'accent', word: 'inspecting' },
	waiting: { icon: 'magnifier', tone: 'attention', word: 'packet waiting for your signature' },
	signed: { icon: 'stamp', tone: 'pass', word: 'signed' },
	notApplicable: { icon: 'hatch', tone: 'muted', word: 'not applicable' },
	needsAnnotation: { icon: 'dashsq', tone: 'muted', word: 'needs annotation' },
	undecided: { icon: 'question', tone: 'attention', word: 'a condition is undecided' }
};

export const STEP: Record<StepState, Look> = {
	notStarted: { icon: 'ring', tone: 'muted', word: 'Not started' },
	copied: { icon: 'paste', tone: 'attention', word: 'Copied' },
	running: { icon: 'dot', tone: 'accent', word: 'Running' },
	done: { icon: 'check', tone: 'pass', word: 'Done' },
	skipped: { icon: 'skip', tone: 'muted', word: 'Skipped' },
	notApplicable: { icon: 'hatch', tone: 'muted', word: 'Not applicable' },
	waitingUntil: { icon: 'clock', tone: 'muted', word: 'Waiting' },
	undecided: { icon: 'question', tone: 'attention', word: 'Undecided' },
	needsAnnotation: { icon: 'dashsq', tone: 'muted', word: 'Needs annotation' },
	needsNewerForeman: { icon: 'warn', tone: 'attention', word: 'Needs a newer Foreman' }
};

export const RUN: Record<RunStatus, Look> = {
	starting: { icon: 'dot', tone: 'accent', word: 'starting' },
	running: { icon: 'dot', tone: 'accent', word: 'running' },
	idle: { icon: 'ring', tone: 'muted', word: 'idle' },
	blocked: { icon: 'warn', tone: 'attention', word: 'waiting for permission' },
	exited: { icon: 'check', tone: 'pass', word: 'exited' },
	errored: { icon: 'cross', tone: 'fail', word: 'errored' },
	interrupted: { icon: 'broken', tone: 'muted', word: 'interrupted' }
};

export const DOCTOR: Record<DoctorLevel, Look> = {
	ok: { icon: 'check', tone: 'pass', word: 'All checks pass' },
	warning: { icon: 'warn', tone: 'attention', word: 'Warnings' },
	failing: { icon: 'cross', tone: 'fail', word: '1 problem' },
	unknown: { icon: 'ring', tone: 'muted', word: 'Not run yet' }
};

/** "fm-tradelog-s02-2-3" style stage number with two digits. */
export const nn = (n: number): string => String(n).padStart(2, '0');
