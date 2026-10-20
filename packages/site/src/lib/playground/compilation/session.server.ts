import { createHmac, randomUUID, timingSafeEqual } from 'node:crypto';

/** Only server-signed IDs may claim an existing compiler sandbox. */
export function compilationSession(cookie: string | undefined, secret: string) {
	const match = cookie?.match(
		/^([0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12})\.([A-Za-z0-9_-]{43})$/
	);
	const sign = (id: string) =>
		createHmac('sha256', secret).update(`playground-session:v1:${id}`).digest();

	if (match) {
		const expected = sign(match[1]);
		const supplied = Buffer.from(match[2], 'base64url');
		if (supplied.length === expected.length && timingSafeEqual(supplied, expected))
			return { id: match[1], cookie: cookie!, fresh: false };
	}

	const id = randomUUID();
	return { id, cookie: `${id}.${sign(id).toString('base64url')}`, fresh: true };
}
