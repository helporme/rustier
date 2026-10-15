import { error } from '@sveltejs/kit';

const clients = new Map<
	string,
	{ started: number; count: number; last: number; pending: number }
>();
const pending = new Set<string>();
let swept = 0;

export function admitCompilation(
	address: string,
	sessionId: string,
	maxPendingPerAddress = 1
): () => void {
	const now = Date.now();

	if (now - swept >= 60_000) {
		for (const [key, client] of clients) {
			if (client.pending === 0 && now - client.started >= 60_000) clients.delete(key);
		}

		swept = now;
	}

	let client = clients.get(address);

	if (
		pending.has(sessionId) ||
		(client && client.pending >= maxPendingPerAddress) ||
		(client && now - client.started < 60_000 && (client.count >= 10 || now - client.last < 1000))
	)
		error(429, 'Too many compilation requests; try again shortly');

	if (!client && clients.size >= 10_000) error(429, 'Compilation admission is at capacity');

	if (!client) {
		client = { started: now, count: 0, last: now, pending: 0 };
		clients.set(address, client);
	} else if (now - client.started >= 60_000) {
		client.started = now;
		client.count = 0;
	}

	client.count++;
	client.last = now;
	client.pending++;
	pending.add(sessionId);
	let released = false;

	return () => {
		if (released) return;

		released = true;
		client.pending--;
		pending.delete(sessionId);
	};
}
