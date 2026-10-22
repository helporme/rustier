import { examples } from './examples';
import { MediaQuery } from 'svelte/reactivity';
import { Inspector } from '#lib/playground/inspector';
import { Compilation } from '#lib/playground/compilation';
import { Terminal } from './terminal.svelte';

export class Playground {
	#desktop = new MediaQuery('(min-width: 48rem)', true);

	sourceTab = $state('lib.rs');
	mobileTab = $state('lib.rs');
	outputTab = $state('js_code');
	selected = $state<string>(examples[0].id);
	drafts = $state(
		Object.fromEntries(
			examples.map((example) => [
				example.id,
				{ rust: String(example.rust), typescript: String(example.typescript) }
			])
		)
	);

	inspector = new Inspector(() => (this.inspect ? this.source.rust : undefined));
	compilation = new Compilation();
	terminal = new Terminal();

	constructor(private readonly getInspect: () => boolean) {}

	run = () => {
		this.terminal.show();
		return this.compilation.run(this.source.rust, this.source.typescript);
	};

	get inspect() {
		return this.getInspect();
	}

	get source() {
		return this.drafts[this.selected];
	}

	get direction() {
		return this.#desktop.current ? 'horizontal' : 'vertical';
	}
}
