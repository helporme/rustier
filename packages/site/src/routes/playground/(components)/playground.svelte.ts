import { examples } from './examples';

export class Playground {
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
}
