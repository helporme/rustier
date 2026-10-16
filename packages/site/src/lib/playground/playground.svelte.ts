import { Builder } from './builder.svelte.ts';
import { Compiler } from './compiler.svelte.ts';
import type { ExampleCompilation } from './compilation-schema';

export class Playground {
	readonly compiler: Compiler;
	readonly builder = new Builder();

	constructor(
		getSource: () => { code: string; typescript: string },
		getExample: () => ExampleCompilation | undefined
	) {
		this.compiler = new Compiler(() => getSource().code);

		$effect(() => {
			const example = getExample();

			if (example) this.builder.seed(example);
		});

		let selected: ReturnType<typeof getSource> | undefined;

		$effect(() => {
			const source = getSource();
			if (source === selected) return;

			selected = source;
			this.builder.clear();
		});
	}
}
