import { Buffer } from 'node:buffer';
import { error } from '@sveltejs/kit';
import { command, getRequestEvent } from '$app/server';
import * as env from '$app/env/private';
import * as v from 'valibot';
import { admitCompilation } from './admission.server';
import { compilationSession } from './session.server';
import {
	BUILD_TIMEOUT_MS,
	MAX_INPUT_BYTES,
	MAX_OUTPUT_BYTES,
	QUEUE_WAIT_MS,
	WORKER_START_TIMEOUT_MS
} from './limits';
import { buildInputSchema, compilerResultSchema, type CompilerResult } from './schema';

export type BuildResult =
	| (Omit<Extract<CompilerResult, { type: 'ok' }>, 'wasm'> & { wasm: Uint8Array<ArrayBuffer> })
	| Extract<CompilerResult, { type: 'err' }>;

const COMPILATION_REQUEST_TIMEOUT_MS =
	QUEUE_WAIT_MS + WORKER_START_TIMEOUT_MS + BUILD_TIMEOUT_MS + 5000;

export const build = command(buildInputSchema, async (input): Promise<BuildResult> => {
	if (!env.COMPILER_URL || !env.COMPILER_TOKEN)
		error(503, 'The compiler service is not configured');

	const event = getRequestEvent();
	const secure = event.url.protocol === 'https:';
	const name = secure ? '__Host-playground-session' : 'playground-session';
	const session = compilationSession(event.cookies.get(name), env.COMPILER_TOKEN);
	const body = JSON.stringify({ ...input, sessionId: session.id });

	if (Buffer.byteLength(body) > MAX_INPUT_BYTES) error(413, 'Compilation request is too large');
	if (session.fresh)
		event.cookies.set(name, session.cookie, {
			path: '/',
			httpOnly: true,
			sameSite: 'strict',
			secure
		});

	const release = admitCompilation(
		event.getClientAddress(),
		session.id,
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

		if (response.status === 429) error(429, 'The compiler is busy; try again shortly');
		if (response.status === 503 || response.status === 504)
			error(503, 'The compiler service is unavailable; try again shortly');
		if (!response.ok)
			error(502, `The compiler service rejected the request (HTTP ${response.status})`);

		let value: unknown;
		try {
			const text = await response.text();
			if (Buffer.byteLength(text) > MAX_OUTPUT_BYTES) error(502, 'Compiler result is too large');
			value = JSON.parse(text);
		} catch {
			error(502, 'The compiler service returned an invalid result');
		}
		const parsed = v.safeParse(compilerResultSchema, value);
		if (!parsed.success) error(502, 'The compiler service returned an invalid result');

		const result = parsed.output;
		return result.type === 'ok'
			? { ...result, wasm: new Uint8Array(Buffer.from(result.wasm, 'base64')) }
			: result;
	} finally {
		release();
	}
});
