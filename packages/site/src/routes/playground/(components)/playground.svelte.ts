import { examples } from './examples/examples';
import { MediaQuery, SvelteURL } from 'svelte/reactivity';
import { Inspector } from '#lib/playground/inspector';
import { Compilation } from '#lib/playground/compilation';
import { Terminal } from './terminal/terminal.svelte';
import { untrack } from 'svelte';
import { createShareUrl, sharedId, type SharedSnippet } from './snippets/share';
import { SavedSnippets } from './snippets/saved-snippets.svelte';

export type PlaygroundSelection = {
	example: string;
	shared: SharedSnippet | null;
};

export class Playground {
	#desktop = new MediaQuery('(min-width: 48rem)', true);
	#selection = '';
	#shared: SharedSnippet | null = null;
	saved = new SavedSnippets();
	saving = $state(false);
	saveError = $state('');
	copied = $state(false);

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

	constructor(
		private readonly getInspect: () => boolean,
		getSelection: () => PlaygroundSelection
	) {
		this.restore(getSelection());
		$effect(() => {
			const selection = getSelection();
			untrack(() => this.restore(selection));
		});
		$effect(() => {
			if (!this.copied) return;
			const timeout = setTimeout(() => (this.copied = false), 2000);
			return () => clearTimeout(timeout);
		});
	}

	private restore({ example, shared }: PlaygroundSelection) {
		const id = shared ? sharedId(shared.code) : example;
		if (id === this.#selection) return;
		this.#selection = id;
		this.#shared = shared;
		if (shared) {
			if (!this.drafts[id]) {
				this.drafts[id] = {
					rust: shared.source.rust,
					typescript: shared.source.typescript
				};
			}
			this.saved.remember(shared);
		}
		this.selected = id;
	}

	get title() {
		return (
			examples.find((example) => example.id === this.selected)?.name ??
			this.saved.entries.find((entry) => sharedId(entry.code) === this.selected)?.source.title ??
			this.#shared?.source.title ??
			'Shared snippet'
		);
	}

	save = async (url: URL, title: string, copy: boolean) => {
		if (this.saving) return null;
		this.saving = true;
		this.saveError = '';
		this.copied = false;
		try {
			const source = {
				title: title.trim(),
				rust: this.source.rust,
				typescript: this.source.typescript
			};
			const link = createShareUrl(url, source).then((href) => {
				const code = new SvelteURL(href).searchParams.get('code')!;
				if (!this.saved.remember({ code, source })) throw new Error(this.saved.error);
				return href;
			});
			if (copy) {
				if (!navigator.clipboard?.write || typeof ClipboardItem === 'undefined') {
					await link;
					throw new Error(
						'Copying links requires clipboard access on HTTPS. The snippet was saved.'
					);
				}
				// Start writing before awaiting compression to retain Safari's user activation.
				const text = link.then((href) => new Blob([href], { type: 'text/plain' }));
				await Promise.all([
					text,
					navigator.clipboard.write([new ClipboardItem({ 'text/plain': text })])
				]);
				this.copied = true;
			} else {
				await link;
			}
			return await link;
		} catch (error) {
			this.saveError =
				error instanceof Error && error.name === 'Error'
					? error.message
					: 'Could not copy the link. Check clipboard permissions and try again.';
			return null;
		} finally {
			this.saving = false;
		}
	};

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
