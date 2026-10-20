// ipc/mock.coverage.test.ts (docs/contracts/ipc.md §7.2 rules 1 and 7).
import { describe, expect, it } from 'vitest';

import commands from './bindings/commands.json' with { type: 'json' };
import { COMMAND_MAP, type CommandName } from './contract.ts';
import { isIpcError } from './errors.ts';
import { requests, responses } from './fixtures/index.ts';
import { MockIpcClient } from './mock.ts';

describe('MockIpcClient', () => {
	it('answers every command with its fixture', async () => {
		const mock = new MockIpcClient();
		for (const cmd of commands as CommandName[]) {
			const method = COMMAND_MAP[cmd];
			await expect(mock[method](requests[cmd])).resolves.toEqual(responses[cmd]);
		}
	});

	it('rejects unknown fields with a typed Invalid, like Rust', async () => {
		const mock = new MockIpcClient();
		const err: unknown = await mock.appBootstrap({ extra: 1 } as never).catch((e: unknown) => e);
		expect(isIpcError(err)).toBe(true);
		expect(err).toMatchObject({
			code: 'invalid',
			detail: { kind: 'invalid', field: 'req', rule: 'unknownField' }
		});
	});

	it('returns copies, so callers cannot mutate fixtures', async () => {
		const mock = new MockIpcClient();
		const a = await mock.appBootstrap({});
		a.projects.push({} as never);
		const b = await mock.appBootstrap({});
		expect(b.projects).toHaveLength(0);
	});
});
