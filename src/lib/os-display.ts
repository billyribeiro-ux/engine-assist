// Reduce Transparency and Increase Contrast (AC-XC-23, ADR-014): the webview detects them with
// media queries and reports each change to Rust through `settings_update.osDisplay`; Rust then
// turns the window's effects off. The values aren't stored and Rust never reads the OS itself.
// Reduce Motion needs no report: `prefers-reduced-motion` zeroes the motion tokens in CSS.
import { ipc } from '#lib/ipc/index.ts';

export const QUERIES = {
	reduceTransparency: '(prefers-reduced-transparency: reduce)',
	increaseContrast: '(prefers-contrast: more)'
} as const;

/** Reports now and on every change; returns the function that stops listening. */
export function watchOsDisplay(): () => void {
	const rt = matchMedia(QUERIES.reduceTransparency);
	const hc = matchMedia(QUERIES.increaseContrast);
	const report = (): void => {
		void ipc().then(async (c) => {
			const s = await c.settingsGet({});
			await c
				.settingsUpdate({
					expectRev: s.rev,
					patch: {
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
						osDisplay: { reduceTransparency: rt.matches, increaseContrast: hc.matches }
					}
				})
				.catch(() => undefined);
		});
	};
	report();
	rt.addEventListener('change', report);
	hc.addEventListener('change', report);
	return () => {
		rt.removeEventListener('change', report);
		hc.removeEventListener('change', report);
	};
}
