import { error } from '@sveltejs/kit';
import { Buffer } from 'node:buffer';
import type { CompilerResult } from '#lib/playground/compilation-schema';
import { examples } from '#lib/playground/examples';
import { highlight } from '#lib/ui/highlight';
import type { PageServerLoad } from './$types';

const compiledExamples = import.meta.glob<{
	code: string;
	result: Extract<CompilerResult, { type: 'ok' }>;
}>('/.rustier/example-results/*.json', { eager: true, import: 'default' });

export const load = (({ params }) => {
	const selected = params.example
		? examples.findIndex((example) => example.slug === params.example)
		: 0;

	if (selected === -1) {
		error(404, 'example not found');
	}

	const example = examples[selected];
	const compiled = compiledExamples[`/.rustier/example-results/${example.slug}.json`];

	return {
		selected,
		compiled:
			compiled?.code === example.code
				? {
						code: compiled.code,
						result: {
							...compiled.result,
							wasm: new Uint8Array(Buffer.from(compiled.result.wasm, 'base64'))
						}
					}
				: undefined,
		preview: {
			rust: {
				value: example.code,
				language: 'rust' as const,
				tokens: highlight(example.code, 'rust')
			},
			typescript: {
				value: example.typescript,
				language: 'typescript' as const,
				tokens: highlight(example.typescript, 'typescript')
			}
		}
	};
}) satisfies PageServerLoad;
