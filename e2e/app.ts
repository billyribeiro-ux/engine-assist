// Launches the built e2e app and opens a WebDriver session on its embedded server.
import { type ChildProcess, spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { join } from 'node:path';

import { WebDriver, waitForServer } from './webdriver.ts';

const root = new URL('..', import.meta.url).pathname;

/** The debug binary of `foreman-app` made with `--features e2e` (see e2e/run.mjs). */
export function appBinary(env: NodeJS.ProcessEnv = process.env): string {
	return (
		env['FOREMAN_E2E_APP'] ?? join(env['CARGO_TARGET_DIR'] ?? join(root, 'target'), 'debug', 'foreman-app')
	);
}

/** A free loopback port, so the suite never collides with another listener. */
export function freePort(): Promise<number> {
	return new Promise((resolve, reject) => {
		const server = createServer();
		server.once('error', reject);
		server.listen(0, '127.0.0.1', () => {
			const address = server.address();
			const port = typeof address === 'object' && address !== null ? address.port : 0;
			server.close(() => {
				resolve(port);
			});
		});
	});
}

export interface RunningApp {
	driver: WebDriver;
	/** Ends the session and stops the app. Safe to call twice. */
	stop(): Promise<void>;
}

/**
 * Starts the app (`TAURI_WEBDRIVER_PORT` tells the plugin where to listen), waits for its WebDriver
 * server and window, and opens a session.
 */
export async function startApp(startTimeoutMs = 90_000): Promise<RunningApp> {
	const port = await freePort();
	const child: ChildProcess = spawn(appBinary(), [], {
		env: { ...process.env, TAURI_WEBDRIVER_PORT: String(port) },
		stdio: ['ignore', 'pipe', 'pipe']
	});
	let exited = false;
	let log = '';
	child.once('exit', () => {
		exited = true;
	});
	for (const stream of [child.stdout, child.stderr]) {
		stream?.on('data', (chunk: Buffer) => {
			log = (log + chunk.toString()).slice(-4000);
		});
	}
	const kill = async (): Promise<void> => {
		if (exited) return;
		child.kill('SIGTERM');
		await new Promise<void>((resolve) => {
			const timer = setTimeout(() => {
				child.kill('SIGKILL');
				resolve();
			}, 5000);
			child.once('exit', () => {
				clearTimeout(timer);
				resolve();
			});
		});
	};
	const baseUrl = `http://127.0.0.1:${port}`;
	try {
		await waitForServer(baseUrl, startTimeoutMs, () => !exited);
		const driver = await WebDriver.create(baseUrl);
		return {
			driver,
			stop: async () => {
				try {
					await driver.deleteSession();
				} catch {
					// the app may already be gone
				}
				await kill();
			}
		};
	} catch (e) {
		await kill();
		throw new Error(`${e instanceof Error ? e.message : String(e)}\napp output (tail):\n${log}`, {
			cause: e
		});
	}
}
