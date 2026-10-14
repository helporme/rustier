import { error } from '@sveltejs/kit';
import { examples } from '#lib/playground/examples';
import { highlight } from '#lib/ui/highlight';
import type { PageServerLoad } from './$types';

export const load = (({ params }) => {
	const selected = params.example
		? examples.findIndex((example) => example.slug === params.example)
		: 0;

	if (selected === -1) {
		error(404, 'example not found');
	}

	const example = examples[selected];

	return {
		selected,
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
