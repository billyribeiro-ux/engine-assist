// See https://svelte.dev/docs/kit/types#app.d.ts
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}

	interface ImportMetaEnv {
		/** `mock` selects MockIpcClient (ADR-004); anything else uses TauriIpcClient. */
		readonly VITE_IPC?: string;
	}
}

export {};
