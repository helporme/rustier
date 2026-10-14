import { Builder } from './builder.svelte.ts';
import { Compiler } from './compiler.svelte.ts';

export class Playground {
	readonly compiler: Compiler;
	readonly builder = new Builder();

	constructor(getSource: () => { code: string; typescript: string }) {
		this.compiler = new Compiler(() => getSource().code);

		let selected: ReturnType<typeof getSource> | undefined;

		$effect(() => {
			const source = getSource();
			if (source === selected) return;

			selected = source;
			this.builder.clear();
		});
	}
}
