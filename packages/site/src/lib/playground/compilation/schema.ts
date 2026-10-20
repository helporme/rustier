import * as v from 'valibot';
import { MAX_INPUT_BYTES } from './limits.ts';

const sourceSchema = v.pipe(v.string(), v.maxLength(MAX_INPUT_BYTES));

export const buildInputSchema = v.object({
	code: sourceSchema,
	typescript: sourceSchema
});

export const compilationRequestSchema = v.object({
	...buildInputSchema.entries,
	sessionId: v.pipe(v.string(), v.uuid())
});

const durationSchema = v.pipe(v.number(), v.finite(), v.minValue(0));
export const compilerResultSchema = v.variant('type', [
	v.object({
		type: v.literal('ok'),
		wasm: v.string(),
		bundle: v.string(),
		wasmModuleId: v.string(),
		duration: durationSchema
	}),
	v.object({
		type: v.literal('err'),
		message: v.string(),
		duration: durationSchema
	})
]);

export type BuildInput = v.InferOutput<typeof buildInputSchema>;
export type CompilationRequest = v.InferOutput<typeof compilationRequestSchema>;
export type CompilerResult = v.InferOutput<typeof compilerResultSchema>;
