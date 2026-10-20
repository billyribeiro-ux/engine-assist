// Browser component tests render screens outside the SvelteKit router, so navigation is a spy.
import { vi } from 'vitest';

vi.mock('$app/navigation', () => ({
	goto: vi.fn(() => Promise.resolve()),
	invalidate: vi.fn(() => Promise.resolve()),
	refreshAll: vi.fn(() => Promise.resolve()),
	afterNavigate: vi.fn(),
	beforeNavigate: vi.fn(),
	onNavigate: vi.fn()
}));
