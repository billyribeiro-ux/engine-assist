// Every in-app link, resolved through $app/paths (SvelteKit 3 routing).
import { resolve } from '$app/paths';

export const to = {
	today: () => resolve('/'),
	firstRun: () => resolve('/first-run'),
	setup: () => resolve('/setup'),
	wizard: () => resolve('/setup/wizard'),
	doctor: () => resolve('/doctor'),
	runLog: () => resolve('/run-log'),
	settings: () => resolve('/settings'),
	templateHealth: () => resolve('/template-health'),
	project: (projectId: string) => resolve('/projects/[projectId]', { projectId }),
	stage: (projectId: string, stage: number) =>
		resolve('/projects/[projectId]/stages/[stage]', { projectId, stage: String(stage) }),
	step: (projectId: string, stage: number, step: string) =>
		resolve('/projects/[projectId]/stages/[stage]/steps/[step]', {
			projectId,
			stage: String(stage),
			step
		}),
	gate: (projectId: string, stage: number) =>
		resolve('/projects/[projectId]/stages/[stage]/gate', { projectId, stage: String(stage) }),
	packet: (projectId: string, stage: number) =>
		resolve('/projects/[projectId]/stages/[stage]/packet', { projectId, stage: String(stage) }),
	changes: (projectId: string) => resolve('/projects/[projectId]/changes', { projectId }),
	assembler: (projectId: string) => resolve('/projects/[projectId]/assembler', { projectId })
};
