import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

// ADR-002: SvelteKit 3 config lives here (svelte.config.js is not supported in kit 3).
// SPA mode: adapter-static with an index.html fallback; ssr = false in src/routes/+layout.ts.
// version.pollInterval: 0 disables the hourly version.json polling (SR-20, S-0 checks the rest).
// SvelteKit's own content-security-policy option stays unset: Tauri hash-pins the bootstrap (ADR-014).
export default defineConfig({
	plugins: [
		sveltekit({
			adapter: adapter({ fallback: 'index.html' }),
			version: { pollInterval: 0 }
		})
	],
	clearScreen: false,
	server: {
		port: 5173,
		strictPort: true,
		host: 'localhost'
	},
	envPrefix: ['VITE_'],
	build: {
		target: 'safari18',
		sourcemap: false
	}
});
