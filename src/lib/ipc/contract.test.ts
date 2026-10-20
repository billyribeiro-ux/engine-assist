/// <reference types="node" />
// ipc/contract.test.ts (ADR-004, docs/contracts/ipc.md §7.1).
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

import commands from './bindings/commands.json' with { type: 'json' };
import { COMMAND_MAP } from './contract.ts';
import { IPC_ERROR_CODES } from './errors.ts';
import { MockIpcClient } from './mock.ts';

const bindingsDir = join(import.meta.dirname, 'bindings');
const ipcMd = readFileSync(join(import.meta.dirname, '../../../docs/contracts/ipc.md'), 'utf8');

/** Command names from §4.15's table in docs/contracts/ipc.md. */
function contractCommands(): string[] {
	const section = ipcMd.split('### 4.15')[1]?.split('## 5.')[0] ?? '';
	const names = new Set<string>();
	for (const row of section.split('\n').filter((l) => l.startsWith('| ') && !l.includes('**'))) {
		for (const m of row.matchAll(/`([a-z_]+)`/g)) if (m[1]) names.add(m[1]);
	}
	return [...names].sort();
}

describe('ipc contract', () => {
	it('COMMAND_MAP keys equal the 91 commands of §4.15', () => {
		const contract = contractCommands();
		expect(contract).toHaveLength(91);
		expect(Object.keys(COMMAND_MAP).sort()).toEqual(contract);
	});

	it('every command Rust registers (bindings/commands.json) is in COMMAND_MAP', () => {
		// Rust adds commands ticket by ticket (M2-01 ff.); the TS contract carries all 91 already.
		for (const c of commands) expect(Object.keys(COMMAND_MAP)).toContain(c);
	});

	it('MockIpcClient has a method for every command', () => {
		const mock = new MockIpcClient();
		for (const method of Object.values(COMMAND_MAP)) {
			expect(typeof mock[method]).toBe('function');
		}
		expect(new Set(Object.values(COMMAND_MAP)).size).toBe(91);
	});

	it('ipc::error_codes_closed: the TS union equals the Rust enum', () => {
		const ts = readFileSync(join(bindingsDir, 'IpcErrorCode.ts'), 'utf8');
		const codes = [...ts.matchAll(/"([A-Za-z]+)"/g)].map((m) => m[1]);
		expect(codes).toEqual([...IPC_ERROR_CODES]);
	});

	it('ipc::no_bigint_in_bindings (IPC-R4)', () => {
		for (const f of readdirSync(bindingsDir).filter((n) => n.endsWith('.ts'))) {
			expect(readFileSync(join(bindingsDir, f), 'utf8'), f).not.toMatch(/\bbigint\b/);
		}
	});

	it('ipc::security_confirmations_are_native (static part over the TS contract too)', () => {
		const src = readFileSync(join(import.meta.dirname, 'contract.ts'), 'utf8');
		const reqs =
			src.split('export interface Requests')[1]?.split('export interface Responses')[0] ?? '';
		expect(reqs).not.toMatch(/\b(confirm\w*|ack\w*|consent\w*|approved\w*|\w*Confirmed)\s*:/);
	});

	it('ipc::security_confirmations_are_native (static part, IPC-R11)', () => {
		const banned = /\b(confirm\w*|ack\w*|consent\w*|approved\w*|\w*Confirmed)\s*:/;
		for (const f of readdirSync(bindingsDir).filter((n) => n.endsWith('Req.ts'))) {
			expect(readFileSync(join(bindingsDir, f), 'utf8'), f).not.toMatch(banned);
		}
	});
});
