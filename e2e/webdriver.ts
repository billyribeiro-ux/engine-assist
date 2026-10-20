// A minimal W3C WebDriver client for the real-app smoke (ADR-015 layer 5, amended 2026-10-20).
//
// It talks to the server embedded in the app by `tauri-plugin-wdio-webdriver` 1.5.0 (HTTP on
// 127.0.0.1:$TAURI_WEBDRIVER_PORT). Endpoints used, checked against the published crate's
// `server/router.rs`: GET /status, POST /session, DELETE /session/{id}, GET|POST /session/{id}/url,
// POST /session/{id}/execute/{sync,async}, POST /session/{id}/element,
// GET /session/{id}/element/{eid}/{text,displayed}, GET /session/{id}/window/handles.
// Node 22 only (global `fetch`), no dependencies. It replaces WebdriverIO, whose dependency tree
// failed the repo's audit, license and trust gates (ADR-015, amendment 2026-10-20).

/** The W3C key under which an element reference is returned. */
const ELEMENT_KEY = 'element-6066-11e4-a52e-4f735466cecf';

/** A W3C error response: `{ value: { error, message, stacktrace } }`. */
export class WebDriverError extends Error {
	readonly code: string;
	readonly status: number;

	constructor(code: string, message: string, status: number) {
		super(`${code}: ${message}`);
		this.name = 'WebDriverError';
		this.code = code;
		this.status = status;
	}
}

export interface ClientOptions {
	/** Per-request timeout (default 60 s; a script can run as long as the server's script timeout). */
	requestTimeoutMs?: number;
}

const isRecord = (v: unknown): v is Record<string, unknown> =>
	typeof v === 'object' && v !== null && !Array.isArray(v);

/** One HTTP call; returns the W3C `value` or throws a `WebDriverError`. */
async function call(
	baseUrl: string,
	method: 'GET' | 'POST' | 'DELETE',
	path: string,
	body: unknown,
	timeoutMs: number
): Promise<unknown> {
	const init: RequestInit = { method, signal: AbortSignal.timeout(timeoutMs) };
	if (method === 'POST') {
		init.headers = { 'Content-Type': 'application/json' };
		init.body = JSON.stringify(body ?? {});
	}
	const response = await fetch(`${baseUrl}${path}`, init);
	const text = await response.text();
	let parsed: unknown;
	try {
		parsed = text === '' ? {} : JSON.parse(text);
	} catch {
		throw new WebDriverError('invalid response', `${method} ${path}: not JSON: ${text.slice(0, 200)}`, response.status);
	}
	const value = isRecord(parsed) ? parsed['value'] : undefined;
	if (!response.ok) {
		const err = isRecord(value) ? value : {};
		throw new WebDriverError(
			typeof err['error'] === 'string' ? err['error'] : 'unknown error',
			typeof err['message'] === 'string' ? err['message'] : `${method} ${path} failed`,
			response.status
		);
	}
	return value;
}

/** Waits until the embedded server answers `/status` with `ready: true` (a webview exists). */
export async function waitForServer(
	baseUrl: string,
	timeoutMs: number,
	isAlive: () => boolean = () => true
): Promise<void> {
	const deadline = Date.now() + timeoutMs;
	let last = 'no response';
	while (Date.now() < deadline) {
		if (!isAlive()) throw new Error('the app exited before the WebDriver server was ready');
		try {
			const value = await call(baseUrl, 'GET', '/status', undefined, 2000);
			if (isRecord(value) && value['ready'] === true) return;
			last = 'server up, webview not ready';
		} catch (e) {
			last = e instanceof Error ? e.message : String(e);
		}
		await new Promise((resolve) => setTimeout(resolve, 250));
	}
	throw new Error(`WebDriver server at ${baseUrl} not ready after ${timeoutMs} ms (${last})`);
}

export class Element {
	readonly #driver: WebDriver;
	readonly id: string;

	constructor(driver: WebDriver, id: string) {
		this.#driver = driver;
		this.id = id;
	}

	async text(): Promise<string> {
		return String(await this.#driver.request('GET', `/element/${this.id}/text`));
	}

	async isDisplayed(): Promise<boolean> {
		return (await this.#driver.request('GET', `/element/${this.id}/displayed`)) === true;
	}
}

export class WebDriver {
	readonly #baseUrl: string;
	readonly #timeoutMs: number;
	readonly sessionId: string;

	private constructor(baseUrl: string, sessionId: string, timeoutMs: number) {
		this.#baseUrl = baseUrl;
		this.sessionId = sessionId;
		this.#timeoutMs = timeoutMs;
	}

	/** Creates a session on the embedded server (`POST /session`). */
	static async create(baseUrl: string, options: ClientOptions = {}): Promise<WebDriver> {
		const timeoutMs = options.requestTimeoutMs ?? 60_000;
		const value = await call(baseUrl, 'POST', '/session', { capabilities: {} }, timeoutMs);
		const id = isRecord(value) ? value['sessionId'] : undefined;
		if (typeof id !== 'string' || id === '') {
			throw new WebDriverError('session not created', 'no sessionId in the response', 500);
		}
		return new WebDriver(baseUrl, id, timeoutMs);
	}

	/** Session-scoped request. Public for `Element`; specs use the typed methods. */
	request(method: 'GET' | 'POST' | 'DELETE', path: string, body?: unknown): Promise<unknown> {
		return call(this.#baseUrl, method, `/session/${this.sessionId}${path}`, body, this.#timeoutMs);
	}

	async deleteSession(): Promise<void> {
		await call(this.#baseUrl, 'DELETE', `/session/${this.sessionId}`, undefined, this.#timeoutMs);
	}

	async getUrl(): Promise<string> {
		return String(await this.request('GET', '/url'));
	}

	async navigateTo(url: string): Promise<void> {
		await this.request('POST', '/url', { url });
	}

	async getWindowHandles(): Promise<string[]> {
		const value = await this.request('GET', '/window/handles');
		return Array.isArray(value) ? value.map(String) : [];
	}

	/**
	 * Runs a script body (W3C "Execute Script"): `return` a value, read arguments from `arguments`.
	 * This server runs the body as an async function, so `await` works too.
	 */
	async execute<T>(script: string, args: unknown[] = []): Promise<T> {
		return (await this.request('POST', '/execute/sync', { script, args })) as T;
	}

	/** Runs a script body whose last argument is the completion callback (W3C "Execute Async Script"). */
	async executeAsync<T>(script: string, args: unknown[] = []): Promise<T> {
		return (await this.request('POST', '/execute/async', { script, args })) as T;
	}

	/** Finds the first element matching a CSS selector, or rejects with `no such element`. */
	async findElement(css: string): Promise<Element> {
		const value = await this.request('POST', '/element', { using: 'css selector', value: css });
		const id = isRecord(value) ? value[ELEMENT_KEY] : undefined;
		if (typeof id !== 'string') {
			throw new WebDriverError('no such element', `no element reference for ${css}`, 404);
		}
		return new Element(this, id);
	}

	/** Polls until `check` returns a truthy value or the timeout passes (errors count as "not yet"). */
	async waitUntil<T>(
		check: () => Promise<T | false | null | undefined>,
		options: { timeoutMs: number; intervalMs?: number; message: string }
	): Promise<T> {
		const deadline = Date.now() + options.timeoutMs;
		let lastError = '';
		for (;;) {
			try {
				const result = await check();
				if (result) return result;
			} catch (e) {
				lastError = e instanceof Error ? e.message : String(e);
			}
			if (Date.now() >= deadline) {
				throw new Error(`${options.message}${lastError ? ` (last error: ${lastError})` : ''}`);
			}
			await new Promise((resolve) => setTimeout(resolve, options.intervalMs ?? 250));
		}
	}

	/** `findElement` plus `isDisplayed`, retried until the timeout. */
	async waitForDisplayed(css: string, timeoutMs = 30_000): Promise<Element> {
		return this.waitUntil(
			async () => {
				const element = await this.findElement(css);
				return (await element.isDisplayed()) ? element : false;
			},
			{ timeoutMs, message: `${css} not displayed within ${timeoutMs} ms` }
		);
	}
}
