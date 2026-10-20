import { error } from '@sveltejs/kit';

const REQUESTS_PER_MINUTE = 120;
const RATE_WINDOW_MS = 60_000;
const clients = new Map<string, { pending: number; started: number; count: number }>();
const sessions = new Set<string>();

/** Bound concurrent work and request frequency before reaching the compiler pool. */
export function admitCompilation(address: string, session: string, maxPending: number) {
	const now = Date.now();
	for (const [key, client] of clients) {
		if (client.pending === 0 && now - client.started >= RATE_WINDOW_MS) clients.delete(key);
	}

	let client = clients.get(address);
	if (sessions.has(session) || (client && client.pending >= maxPending))
		error(429, 'A compilation is already pending; wait for it to finish');
	if (client && now - client.started < RATE_WINDOW_MS && client.count >= REQUESTS_PER_MINUTE)
		error(429, `Too many compilation requests; limit is ${REQUESTS_PER_MINUTE} per minute`);
	if (!client && clients.size >= 10_000) error(429, 'The compiler is busy; try again shortly');

	if (!client) {
		client = { pending: 0, started: now, count: 0 };
		clients.set(address, client);
	}
	if (now - client.started >= RATE_WINDOW_MS) {
		client.started = now;
		client.count = 0;
	}
	client.pending++;
	client.count++;
	sessions.add(session);

	return () => {
		client.pending--;
		sessions.delete(session);
	};
}
