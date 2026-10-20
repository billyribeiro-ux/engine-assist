/** The project slug: lowercase letters, digits and hyphens, 1–40 (setup_preview, DR-121). */
export function slugOf(name: string): string {
	return name
		.toLowerCase()
		.normalize('NFKD')
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '')
		.slice(0, 40);
}
