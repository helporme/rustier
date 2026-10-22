import type { PageLoad } from './$types';
import { error } from '@sveltejs/kit';
import { decodeSharedSource } from '../(components)/snippets/share';
import { examples } from '../(components)/examples/examples';

export const load: PageLoad = async ({ url, params }) => {
	const code = url.searchParams.get('code');
	if (code === null) {
		const example = params.slug
			? examples.find((example) => example.id === params.slug)
			: examples[0];
		if (!example) error(404, 'Example not found');
		return { selection: { example: example.id, shared: null }, shareError: '' };
	}
	try {
		const source = await decodeSharedSource(code);
		return {
			selection: { example: examples[0].id, shared: { code, source } },
			shareError: ''
		};
	} catch {
		return {
			selection: { example: examples[0].id, shared: null },
			shareError: 'Could not open this share link. It may be invalid or too large.'
		};
	}
};
