// What a step row offers the workspace's shortcuts (⌘↵, ⌘⇧C, ⌘⌥C, ⌘⇧V, ⌘D).
export interface RowApi {
	run(): Promise<true | string>;
	copy(target: 'prompt' | 'command'): Promise<true | string>;
	paste(): Promise<true | string>;
	markDone(): Promise<true | string>;
}
