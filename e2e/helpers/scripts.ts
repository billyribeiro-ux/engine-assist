// Page scripts for the smoke specs, as plain-JavaScript strings. WebdriverIO would otherwise
// serialize TypeScript functions after tsx has compiled them (with helper calls such as `__name`
// that do not exist in the page), so the page code stays a string and is never compiled.

/** The Today heading. The one place to change if M2 reshapes the Today route. */
export const TODAY_HEADING = 'Today';

/** Where a same-document check can read the app's state (sync script, returns JSON-safe data). */
export const READ_STATE = `
	var h1 = document.querySelector('h1');
	return {
		href: location.href,
		origin: location.origin,
		heading: h1 ? h1.textContent.trim() : null,
		alert: document.querySelector('[role="alert"]') ? document.querySelector('[role="alert"]').textContent.trim() : null,
		marker: window.__fmMarker === undefined ? null : String(window.__fmMarker),
		tauriInternals: typeof window.__TAURI_INTERNALS__,
		mockIpc: typeof window.__FOREMAN_MOCK_IPC__
	};
`;

/** Tags this document so a navigation (which replaces the document) is detectable. */
export const SET_MARKER = `window.__fmMarker = 'alive'; return location.href;`;

/**
 * SR-20: appends an inline <script> and reports every securitypolicyviolation event raised for it,
 * and whether the script ran. The embedded WebDriver server runs a script body as an async
 * function (tauri-plugin-wdio-webdriver, `execute/sync`), so `await` and a returned value work.
 */
export const INJECT_INLINE_SCRIPT = `
	var seen = [];
	function onViolation(e) {
		seen.push({
			effectiveDirective: e.effectiveDirective,
			violatedDirective: e.violatedDirective,
			blockedURI: e.blockedURI,
			disposition: e.disposition,
			originalPolicy: e.originalPolicy
		});
	}
	document.addEventListener('securitypolicyviolation', onViolation);
	delete window.__fmInjected;
	var el = document.createElement('script');
	el.textContent = "window.__fmInjected = 'executed';";
	document.head.appendChild(el);
	await new Promise(function (resolve) { setTimeout(resolve, 500); });
	document.removeEventListener('securitypolicyviolation', onViolation);
	var executed = window.__fmInjected === 'executed';
	el.remove();
	delete window.__fmInjected;
	return { violations: seen, executed: executed };
`;

/** SR-29 attempts. Each takes the target URL as arguments[0] and reports what the call returned. */
export const NAVIGATION_ATTEMPTS = {
	'window.open(_blank)': `
		try {
			var w = window.open(arguments[0], '_blank');
			return w === null ? 'null' : 'window';
		} catch (e) { return 'threw ' + String(e); }
	`,
	'window.open(_self)': `
		try { window.open(arguments[0], '_self'); return 'called'; } catch (e) { return 'threw ' + String(e); }
	`,
	'location.assign': `
		try { location.assign(arguments[0]); return 'called'; } catch (e) { return 'threw ' + String(e); }
	`,
	'anchor click (target=_blank)': `
		var a = document.createElement('a');
		a.href = arguments[0];
		a.target = '_blank';
		document.body.appendChild(a);
		a.click();
		a.remove();
		return 'clicked';
	`,
	'anchor click (same frame)': `
		var a = document.createElement('a');
		a.href = arguments[0];
		document.body.appendChild(a);
		a.click();
		a.remove();
		return 'clicked';
	`
} as const;

/**
 * Positive control: navigation within the app origin is still allowed (it reloads the document).
 * Deferred, so the script's result is read back before the document is replaced.
 */
export const RELOAD_SAME_ORIGIN = `
	var target = new URL('/', location.href).href;
	setTimeout(function () { location.assign(target); }, 100);
	return 'scheduled';
`;
