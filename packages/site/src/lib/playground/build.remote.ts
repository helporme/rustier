import { Buffer } from 'node:buffer';
import { error } from '@sveltejs/kit';
import { command, getRequestEvent } from '$app/server';
import * as env from '$app/env/private';
import * as v from 'valibot';
import { admitCompilation } from '../server/compilation-admission';
import { compilationSession } from '../server/compilation-session';
import {
	COMPILATION_REQUEST_TIMEOUT_MS,
	MAX_INPUT_BYTES,
	buildInputSchema,
	compilerResultSchema,
	type BuildResult
} from './compilation-schema';

export type { BuildResult } from './compilation-schema';

export const build = command(buildInputSchema, async (input): Promise<BuildResult> => {
	if (!env.COMPILER_URL || !env.COMPILER_TOKEN)
		error(503, 'The compiler service is not configured');

	const event = getRequestEvent();
	const secure = event.url.protocol === 'https:';
	const cookieName = secure ? '__Host-playground-session' : 'playground-session';
	const session = compilationSession(event.cookies.get(cookieName), env.COMPILER_TOKEN);
	const sessionId = session.id;

	if (session.fresh) {
		event.cookies.set(cookieName, session.cookie, {
			path: '/',
			httpOnly: true,
			sameSite: 'strict',
			secure
		});
	}

	const body = JSON.stringify({ ...input, sessionId });

	if (Buffer.byteLength(body) > MAX_INPUT_BYTES) error(413, 'Compilation request is too large');

	const release = admitCompilation(
		event.getClientAddress(),
		sessionId,
		env.COMPILER_MAX_PENDING_PER_IP
	);

	try {
		let response: Response;

		try {
			response = await fetch(new URL('/build', env.COMPILER_URL), {
				method: 'POST',
				headers: {
					'content-type': 'application/json',
					authorization: `Bearer ${env.COMPILER_TOKEN}`
				},
				body,
				signal: AbortSignal.any([
					event.request.signal,
					AbortSignal.timeout(COMPILATION_REQUEST_TIMEOUT_MS)
				])
			});
		} catch {
			error(503, 'The compiler service is unavailable');
		}

		if (!response.ok) {
			if (response.status === 429) error(429, 'The compiler is busy; try again shortly');
			if (response.status === 503 || response.status === 504)
				error(503, 'The compiler service is unavailable; try again shortly');

			error(502, 'The compiler service rejected the request');
		}

		const parsed = v.safeParse(compilerResultSchema, await response.json());

		if (!parsed.success) error(502, 'The compiler service returned an invalid result');

		const result = parsed.output;

		return result.type === 'ok'
			? { ...result, wasm: new Uint8Array(Buffer.from(result.wasm, 'base64')) }
			: result;
	} finally {
		release();
	}
});
