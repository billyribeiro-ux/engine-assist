/// <reference types="node" />
// ESLint flat config (ADR-002, ADR-004, ADR-019, SR-26, SR-60, M0-02, M0-03).
// Security rules are proven by planted violations: scripts/lint-plants/ (tests/lint-plants.test.ts).
import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import { defineConfig } from 'eslint/config';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';

const HTML_SINK = 'SR-26: never render content as HTML. Build DOM with Svelte templates instead.';
const SERVER_FILE =
	'ADR-002: Foreman is a client-only SPA. No +page.server, +layout.server, +server, hooks.server or remote files.';

export default defineConfig(
	{
		ignores: [
			'build/',
			'.svelte-kit/',
			'node_modules/',
			'target/',
			'src-tauri/',
			'coverage/',
			'test-results/',
			'playwright-report/',
			'src/lib/ipc/bindings/',
			'scripts/lint-plants/fixtures/'
		]
	},
	js.configs.recommended,
	ts.configs.strictTypeChecked,
	ts.configs.stylisticTypeChecked,
	svelte.configs.recommended,
	prettier,
	svelte.configs.prettier,
	{
		languageOptions: {
			globals: { ...globals.browser, ...globals.node },
			parserOptions: {
				projectService: true,
				tsconfigRootDir: import.meta.dirname,
				extraFileExtensions: ['.svelte']
			}
		}
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
		languageOptions: {
			parserOptions: { parser: ts.parser, svelteFeatures: { experimentalGenerics: true } }
		}
	},
	{
		files: ['**/*.js', '**/*.mjs'],
		extends: [ts.configs.disableTypeChecked]
	},
	{
		rules: {
			// SR-26 / AC-F030-04
			'svelte/no-at-html-tags': 'error',
			'no-restricted-properties': [
				'error',
				{ property: 'innerHTML', message: HTML_SINK },
				{ property: 'outerHTML', message: HTML_SINK },
				{ property: 'insertAdjacentHTML', message: HTML_SINK },
				{ object: 'document', property: 'write', message: HTML_SINK },
				{ object: 'document', property: 'writeln', message: HTML_SINK }
			],
			// ADR-004: only #lib/ipc/tauri.ts talks to Tauri; no plugin JS packages at all (ADR-014).
			'no-restricted-imports': [
				'error',
				{
					patterns: [
						{
							group: ['@tauri-apps/api', '@tauri-apps/api/*'],
							message: 'ADR-004: only src/lib/ipc/tauri.ts may import @tauri-apps/api.'
						},
						{
							group: ['@tauri-apps/plugin-*'],
							message: 'ADR-014: plugins are reached only through Foreman commands in Rust.'
						},
						{ group: ['$lib', '$lib/*'], message: 'ADR-002: use #lib imports.' }
					]
				}
			],
			// SR-60: Foreman makes no network requests of its own.
			'no-restricted-globals': [
				'error',
				{ name: 'fetch', message: 'SR-60: no network. Data comes from Rust over IPC.' },
				{ name: 'XMLHttpRequest', message: 'SR-60: no network.' },
				{ name: 'WebSocket', message: 'SR-60: no network.' },
				{ name: 'EventSource', message: 'SR-60: no network.' }
			],
			'@typescript-eslint/consistent-type-imports': 'error',
			// Numbers in template literals are routine in UI text ("Stage 2 of 19").
			'@typescript-eslint/restrict-template-expressions': ['error', { allowNumber: true }]
		}
	},
	{
		files: ['src/lib/ipc/tauri.ts'],
		rules: {
			'no-restricted-imports': [
				'error',
				{ patterns: [{ group: ['@tauri-apps/plugin-*'], message: 'ADR-014' }] }
			]
		}
	},
	{
		files: [
			'**/+page.server.*',
			'**/+layout.server.*',
			'**/+server.*',
			'src/hooks.server.*',
			'**/*.remote.*',
			'**/*.server.*',
			'**/server/**'
		],
		ignores: ['scripts/**', 'tests/**'],
		rules: {
			'no-restricted-syntax': ['error', { selector: 'Program', message: SERVER_FILE }]
		}
	},
	{
		// Tooling scripts run in Node, outside the webview; they may use the network-free Node APIs.
		files: ['scripts/**', '*.config.*', 'tests/**'],
		rules: { 'no-restricted-globals': 'off' }
	}
);
