import { examples } from './examples';
import { MediaQuery } from 'svelte/reactivity';

export class Playground {
	#desktop = new MediaQuery('(min-width: 48rem)', true);

	selected = $state('counter');
	drafts = $state(
		Object.fromEntries(
			examples.map((example) => [
				example.id,
				{ rust: String(example.rust), typescript: String(example.typescript) }
			])
		)
	);

	get source() {
		return this.drafts[this.selected];
	}

	get direction() {
		return this.#desktop.current ? 'horizontal' : 'vertical';
	}
}
