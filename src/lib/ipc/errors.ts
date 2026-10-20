import type { IpcError } from './bindings/IpcError.ts';
import type { IpcErrorCode } from './bindings/IpcErrorCode.ts';
import type { IpcErrorDetail } from './bindings/IpcErrorDetail.ts';

/** Every `IpcErrorCode`, in Rust declaration order (`ipc::error_codes_closed`). */
export const IPC_ERROR_CODES = [
	'invalid',
	'notFound',
	'expired',
	'stale',
	'readOnly',
	'notAllowed',
	'conflict',
	'policyViolation',
	'toolMissing',
	'toolFailed',
	'clipboard',
	'cancelled',
	'unsupported',
	'io',
	'internal'
] as const satisfies readonly IpcErrorCode[];

// Compile-time proof that IPC_ERROR_CODES covers the whole union.
type AssertNever<T extends never> = T;
export type IpcErrorCodesExhaustive = AssertNever<
	Exclude<IpcErrorCode, (typeof IPC_ERROR_CODES)[number]>
>;

/** Foreman-written text for each code; the UI never parses `message` (ADR-004 §5). */
export const IPC_ERROR_TEXT: Record<IpcErrorCode, string> = {
	invalid: 'The request was not valid.',
	notFound: 'That item no longer exists.',
	expired: 'That preview expired. Try again.',
	stale: 'Something changed on disk. Review the fresh version.',
	readOnly: 'This project is open read-only.',
	notAllowed: "That action isn't allowed here.",
	conflict: 'A file changed outside Foreman.',
	policyViolation: 'That path is outside what Foreman may touch.',
	toolMissing: 'A required tool is missing.',
	toolFailed: 'A tool failed.',
	clipboard: "The clipboard couldn't be read.",
	cancelled: 'Cancelled.',
	unsupported: "This isn't supported yet.",
	io: 'A file operation failed.',
	internal: 'Something went wrong inside Foreman.'
};

/**
 * What every IpcClient method rejects with: an `Error` that is also a structural `IpcError`
 * (`code`, `message`, `detail`), so UI code branches on `code` and `detail.kind`.
 */
export class IpcFailure extends Error implements IpcError {
	readonly code: IpcErrorCode;
	readonly detail: IpcErrorDetail | null;

	constructor(e: IpcError) {
		super(e.message);
		this.name = 'IpcFailure';
		this.code = e.code;
		this.detail = e.detail;
	}
}

export function isIpcError(value: unknown): value is IpcError {
	if (typeof value !== 'object' || value === null) return false;
	const v = value as { code?: unknown; message?: unknown };
	return (
		typeof v.code === 'string' &&
		(IPC_ERROR_CODES as readonly string[]).includes(v.code) &&
		typeof v.message === 'string'
	);
}

/** Wraps a transport rejection; anything that isn't an IpcError becomes `internal`. */
export function toIpcFailure(value: unknown): IpcFailure {
	if (value instanceof IpcFailure) return value;
	if (isIpcError(value)) return new IpcFailure(value);
	return new IpcFailure({ code: 'internal', message: IPC_ERROR_TEXT.internal, detail: null });
}
