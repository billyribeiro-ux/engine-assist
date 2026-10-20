// The project open in the window (the sidebar's stage list, the strip, the palette and the title
// bar read it) and the selected step in its workspace.
import type { ProjectView, StepSummary } from '#lib/ipc/types.ts';

class Current {
	project = $state<ProjectView | null>(null);
	step = $state<StepSummary | null>(null);
	stage = $state<number | null>(null);
}

export const current = new Current();
