// Mock fixtures (docs/contracts/ipc.md §7.2). The same JSON files are round-tripped by Rust
// (ipc::fixture_requests_deserialize, ipc::fixture_responses_roundtrip), so the mock can't drift.
import type { AppBootstrap } from '../bindings/AppBootstrap.ts';
import type { AppBootstrapReq } from '../bindings/AppBootstrapReq.ts';
import appBootstrapReq from './requests/app_bootstrap.json' with { type: 'json' };
import appBootstrapRes from './responses/app_bootstrap.json' with { type: 'json' };

export const requests = {
	app_bootstrap: appBootstrapReq satisfies AppBootstrapReq
} as const;

export const responses = {
	app_bootstrap: appBootstrapRes as AppBootstrap
} as const;
