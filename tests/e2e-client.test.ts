// Unit tests for the in-repo W3C WebDriver client (e2e/webdriver.ts) against a fake server that
// answers like tauri-plugin-wdio-webdriver 1.5.0 (response shapes taken from its published source).
import { createServer, type Server, type ServerResponse } from 'node:http';
import type { AddressInfo } from 'node:net';

import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import { WebDriver, WebDriverError, waitForServer } from '../e2e/webdriver.ts';

const ELEMENT_KEY = 'element-6066-11e4-a52e-4f735466cecf';

let server: Server;
let base: string;
let ready = false;
let findsBeforeFound = 0;
const seen: { method: string; url: string; body: unknown }[] = [];

function reply(res: ServerResponse, status: number, value: unknown): void {
	res.writeHead(status, { 'Content-Type': 'application/json; charset=utf-8' });
	res.end(JSON.stringify({ value }));
}

beforeAll(async () => {
	server = createServer((req, res) => {
		let raw = '';
		req.on('data', (c: Buffer) => (raw += c.toString()));
		req.on('end', () => {
			const body: unknown = raw === '' ? undefined : JSON.parse(raw);
			const url = req.url ?? '';
			seen.push({ method: req.method ?? '', url, body });
			if (url === '/status') {
				reply(res, 200, { ready, message: 'x' });
				return;
			}
			if (url === '/session' && req.method === 'POST') {
				reply(res, 200, { sessionId: 'S1', capabilities: {} });
				return;
			}
			if (url === '/session/S1' && req.method === 'DELETE') {
				reply(res, 200, null);
				return;
			}
			if (url === '/session/S1/url') {
				reply(res, 200, req.method === 'GET' ? 'tauri://localhost/' : null);
				return;
			}
			if (url === '/session/S1/window/handles') {
				reply(res, 200, ['main']);
				return;
			}
			if (url === '/session/S1/execute/sync') {
				reply(res, 200, { echoed: body });
				return;
			}
			if (url === '/session/S1/execute/async') {
				reply(res, 200, 'async-result');
				return;
			}
			if (url === '/session/S1/element') {
				if (findsBeforeFound-- > 0) {
					reply(res, 404, { error: 'no such element', message: 'Unable to locate element' });
					return;
				}
				reply(res, 200, { [ELEMENT_KEY]: 'E1' });
				return;
			}
			if (url === '/session/S1/element/E1/text') {
				reply(res, 200, 'Today');
				return;
			}
			if (url === '/session/S1/element/E1/displayed') {
				reply(res, 200, true);
				return;
			}
			if (url === '/session/S1/boom') {
				reply(res, 500, { error: 'javascript error', message: 'kaput', stacktrace: '' });
				return;
			}
			reply(res, 404, { error: 'unknown command', message: url });
		});
	});
	await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
	base = `http://127.0.0.1:${String((server.address() as AddressInfo).port)}`;
});

afterAll(() => {
	server.close();
});

describe('e2e WebDriver client', () => {
	it('waits for /status ready, and gives up with a clear message', async () => {
		await expect(waitForServer(base, 600)).rejects.toThrow(/webview not ready/);
		ready = true;
		await waitForServer(base, 2000);
		await expect(waitForServer(base, 2000, () => false)).rejects.toThrow(/exited/);
	});

	it('creates a session, runs scripts, reads URL, windows and elements, and deletes the session', async () => {
		const wd = await WebDriver.create(base);
		expect(wd.sessionId).toBe('S1');
		expect(seen.find((r) => r.url === '/session')?.body).toEqual({ capabilities: {} });

		expect(await wd.getUrl()).toBe('tauri://localhost/');
		await wd.navigateTo('tauri://localhost/x');
		expect(seen.at(-1)).toMatchObject({
			method: 'POST',
			url: '/session/S1/url',
			body: { url: 'tauri://localhost/x' }
		});
		expect(await wd.getWindowHandles()).toEqual(['main']);
		expect(await wd.execute<{ echoed: unknown }>('return 1', ['a', 2])).toEqual({
			echoed: { script: 'return 1', args: ['a', 2] }
		});
		expect(await wd.executeAsync<string>('arguments[0]()')).toBe('async-result');

		const h1 = await wd.findElement('h1');
		expect(seen.at(-1)?.body).toEqual({ using: 'css selector', value: 'h1' });
		expect(await h1.text()).toBe('Today');
		expect(await h1.isDisplayed()).toBe(true);

		await wd.deleteSession();
		expect(seen.at(-1)).toMatchObject({ method: 'DELETE', url: '/session/S1' });
	});

	it('maps W3C errors to WebDriverError', async () => {
		const wd = await WebDriver.create(base);
		const err = await wd.request('GET', '/boom').catch((e: unknown) => e);
		expect(err).toBeInstanceOf(WebDriverError);
		expect(err).toMatchObject({ code: 'javascript error', status: 500 });
		await expect(wd.request('GET', '/nope')).rejects.toMatchObject({ code: 'unknown command' });
	});

	it('retries until an element is displayed, and times out with the last error', async () => {
		const wd = await WebDriver.create(base);
		findsBeforeFound = 2;
		const el = await wd.waitForDisplayed('h1', 5000);
		expect(await el.text()).toBe('Today');
		findsBeforeFound = 1000;
		await expect(wd.waitForDisplayed('h1', 600)).rejects.toThrow(
			/not displayed within 600 ms.*no such element/
		);
	});
});
