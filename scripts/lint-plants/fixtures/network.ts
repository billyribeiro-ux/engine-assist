export async function phoneHome(): Promise<Response> {
	return fetch('https://example.com/telemetry');
}
