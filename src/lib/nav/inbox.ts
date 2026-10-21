// The Today inbox's order (AC-F017-07). Rust sorts it already; the webview keeps the same rule so
// a mock, a stale cache or a merged list can never show a lower-priority row above a higher one.
// Items of one rank keep the order they arrived in (the project order).
import type { InboxItem } from '#lib/ipc/types.ts';

export type InboxKind = InboxItem['kind'];

/** session-blocked, session-your-turn, packet-landed/invalid, gate-failed/signable, outside-change,
 * tamper, chat-awaiting-paste, wait-elapsed, interrupted-session, step-ready. */
export const INBOX_RANK: Record<InboxKind, number> = {
	sessionBlocked: 0,
	sessionYourTurn: 1,
	packetLanded: 2,
	packetInvalid: 2,
	gateFailed: 3,
	gateSignable: 3,
	outsideChange: 4,
	tamper: 5,
	chatAwaitingPaste: 6,
	waitElapsed: 7,
	interruptedSession: 8,
	stepReady: 9
};

/** A stable sort by rank; the input is not changed. */
export function sortInbox(items: readonly InboxItem[]): InboxItem[] {
	return items
		.map((item, n) => ({ item, n }))
		.sort((a, b) => INBOX_RANK[a.item.kind] - INBOX_RANK[b.item.kind] || a.n - b.n)
		.map((x) => x.item);
}

/** What one project shows in the sidebar (AC-F017-10): a problem, something waiting, or all clear. */
export type ProjectLevel = 'problem' | 'waiting' | 'clear';

const PROBLEM: ReadonlySet<InboxKind> = new Set(['packetInvalid', 'gateFailed', 'tamper']);
const WAITING: ReadonlySet<InboxKind> = new Set([
	'sessionBlocked',
	'sessionYourTurn',
	'packetLanded',
	'gateSignable',
	'outsideChange',
	'chatAwaitingPaste',
	'waitElapsed'
]);

export function projectLevel(
	items: readonly InboxItem[],
	projectId: string,
	readOnly: boolean
): ProjectLevel {
	const mine = items.filter((i) => i.project === projectId);
	if (readOnly || mine.some((i) => PROBLEM.has(i.kind))) return 'problem';
	if (mine.some((i) => WAITING.has(i.kind))) return 'waiting';
	return 'clear';
}
