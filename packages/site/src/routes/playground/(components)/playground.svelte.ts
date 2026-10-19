import { examples } from './examples';
import { MediaQuery } from 'svelte/reactivity';
import { Inspector } from '#lib/playground/inspector';

export class Playground {
	#desktop = new MediaQuery('(min-width: 48rem)', true);

	sourceTab = $state('lib.rs');
	outputTab = $state('js_code');
	selected = $state('counter');
	drafts = $state(
		Object.fromEntries(
			examples.map((example) => [
				example.id,
				{ rust: String(example.rust), typescript: String(example.typescript) }
			])
		)
	);

	inspector = new Inspector(() => (this.inspect ? this.source.rust : undefined));

	constructor(private readonly getInspect: () => boolean) {}

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
