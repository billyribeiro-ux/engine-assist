// A settings patch where every field not named stays unchanged (`null`, ipc.md §4.1).
import type { SettingsPatch } from '#lib/ipc/types.ts';

export function patch(p: Partial<SettingsPatch>): SettingsPatch {
	return {
		driName: null,
		editor: null,
		theme: null,
		terminal: null,
		envPassthrough: null,
		globalVariables: null,
		resetTool: null,
		clearLoginPath: null,
		removeAllowlistEntry: null,
		screenReaderMode: null,
		osDisplay: null,
		...p
	};
}
