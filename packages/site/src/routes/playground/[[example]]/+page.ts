import { error } from '@sveltejs/kit';
import { examples } from '#lib/playground/examples';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ params }) => {
	const selected = params.example
		? examples.findIndex((example) => example.slug === params.example)
		: 0;

	if (selected === -1) {
		error(404, 'example not found');
	}

	return { selected };
};
