import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';

/**
 * SR-20, ADR-020: SvelteKit's route announcer (`root.svelte`) has a static `style` attribute, which
 * the production policy (`style-src 'self'`, no 'unsafe-inline') refuses with a violation. The
 * attribute is dropped before the component compiles; base.css hides `#svelte-announcer` the same
 * way. The build fails if the attribute moves, so a SvelteKit update can't bring it back silently.
 */
function announcerWithoutInlineStyle(): Plugin {
	return {
		name: 'foreman:announcer-without-inline-style',
		enforce: 'pre',
		transform(code, id) {
			if (!/@sveltejs[\\/]kit[\\/].*components[\\/]root\.svelte$/.test(id)) return null;
			const out = code.replace(/(id="svelte-announcer"[^>]*?)\s+style="[^"]*"/, '$1');
			if (out === code) this.error('the svelte-announcer style attribute was not found');
			return { code: out, map: null };
		}
	};
}

// ADR-002: SvelteKit 3 config lives here (svelte.config.js is not supported in kit 3).
// SPA mode: adapter-static with an index.html fallback; ssr = false in src/routes/+layout.ts.
// version.pollInterval: 0 disables the hourly version.json polling (SR-20, S-0 checks the rest).
// SvelteKit's own content-security-policy option stays unset: Tauri hash-pins the bootstrap (ADR-014).
export default defineConfig({
	plugins: [
		announcerWithoutInlineStyle(),
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
