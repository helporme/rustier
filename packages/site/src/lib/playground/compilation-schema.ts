import * as v from 'valibot';

export const MAX_INPUT_BYTES = 64 * 1024;
export const MAX_OUTPUT_BYTES = 16 * 1024 * 1024;
export const BUILD_TIMEOUT_MS = 60_000;
export const WORKER_START_TIMEOUT_MS = 60_000;
export const QUEUE_WAIT_MS = 15_000;
export const COMPILATION_REQUEST_TIMEOUT_MS =
	QUEUE_WAIT_MS + WORKER_START_TIMEOUT_MS + BUILD_TIMEOUT_MS + 5000;

export const buildInputSchema = v.object({
	code: v.pipe(
		v.string(),
		v.check((code) => new TextEncoder().encode(code).byteLength <= MAX_INPUT_BYTES)
	)
});

export const compilationRequestSchema = v.object({
	...buildInputSchema.entries,
	sessionId: v.pipe(v.string(), v.uuid())
});

export const compilerResultSchema = v.variant('type', [
	v.object({
		type: v.literal('ok'),
		wasm: v.string(),
		glue: v.string(),
		bundle: v.string(),
		ffiModuleId: v.string(),
		exports: v.record(v.string(), v.string()),
		duration: v.pipe(v.number(), v.finite(), v.minValue(0))
	}),
	v.object({
		type: v.literal('err'),
		message: v.string(),
		duration: v.pipe(v.number(), v.finite(), v.minValue(0))
	})
]);

export type BuildInput = v.InferOutput<typeof buildInputSchema>;
export type CompilationRequest = v.InferOutput<typeof compilationRequestSchema>;
export type CompilerResult = v.InferOutput<typeof compilerResultSchema>;
export type BuildResult =
	| (Omit<Extract<CompilerResult, { type: 'ok' }>, 'wasm'> & { wasm: Uint8Array })
	| Extract<CompilerResult, { type: 'err' }>;

export type ExampleCompilation = {
	code: string;
	result: Extract<BuildResult, { type: 'ok' }>;
};
