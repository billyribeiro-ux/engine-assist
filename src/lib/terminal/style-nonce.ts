// ADR-014, SR-20: the production policy is `style-src 'self'` plus the nonce Tauri gives each
// `<style>` element in the HTML. xterm builds its own `<style>` elements at runtime (cell sizes,
// theme colors); without a nonce the policy refuses them and the terminal draws without its
// layout. While xterm opens, every `<style>` it creates gets the nonce of app.html's carrier
// element. Nothing else: markup that arrives as HTML still can't carry a valid nonce, and
// script-src is untouched.

/** The style nonce Tauri put on app.html's `#fm-style-nonce` (empty outside Tauri). */
export function styleNonce(doc: Document = document): string {
	const el = doc.getElementById('fm-style-nonce');
	if (!(el instanceof HTMLStyleElement)) return '';
	if (el.nonce !== '') return el.nonce;
	return el.getAttribute('nonce') ?? '';
}

/** Runs `open` with `<style>` elements created during it carrying the page's style nonce. */
export function withStyleNonce<T>(open: () => T, doc: Document = document): T {
	const nonce = styleNonce(doc);
	const create: unknown = Reflect.get(doc, 'createElement');
	if (!nonce || typeof create !== 'function') return open();
	const own = Object.hasOwn(doc, 'createElement');
	Reflect.set(doc, 'createElement', function (this: Document, ...args: unknown[]): unknown {
		const el: unknown = Reflect.apply(create, this, args);
		if (el instanceof HTMLStyleElement) el.nonce = nonce;
		return el;
	});
	try {
		return open();
	} finally {
		if (own) Reflect.set(doc, 'createElement', create);
		else Reflect.deleteProperty(doc, 'createElement');
	}
}
