// Mock fixtures (docs/contracts/ipc.md §7.1–7.2). Requests are typed in `requests.ts`; both the
// request and response JSON files are written by `fixtures.test.ts` from the mock and round-tripped
// by Rust (ipc::fixture_requests_deserialize, ipc::fixture_responses_roundtrip), so the mock can't
// drift from Rust's shapes.
import type { CommandName, Responses } from '../contract.ts';

export { requests, requestVariants } from './requests.ts';

const files = import.meta.glob<unknown>('./responses/*.json', { eager: true, import: 'default' });

/** Response fixtures by command (only for files that exist; fixtures.test.ts requires all 91). */
export const responses = Object.fromEntries(
	Object.entries(files).map(([path, json]) => [/([a-z_.]+)\.json$/.exec(path)?.[1] ?? path, json])
) as { [K in CommandName]: Responses[K] };
