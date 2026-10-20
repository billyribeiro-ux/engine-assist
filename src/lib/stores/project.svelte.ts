// The project open in the window (the sidebar's stage list, the strip and the title bar read it).
import type { ProjectView } from '#lib/ipc/types.ts';

class Current {
	project = $state<ProjectView | null>(null);
}

export const current = new Current();
