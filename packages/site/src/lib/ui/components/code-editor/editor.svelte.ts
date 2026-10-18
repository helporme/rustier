import { untrack } from 'svelte';
import type { CodeMirror, EditorOptions } from './codemirror';

export class CodeEditorController {
	ready = $state(false);
	failed = $state(false);
	#editor = $state.raw<CodeMirror>();
	#options: EditorOptions;

	constructor(options: EditorOptions) {
		this.#options = options;

		$effect(() => {
			const editor = this.#editor;
			const value = options.value;
			if (editor) untrack(() => editor.setValue(value));
		});

		$effect(() => {
			const editor = this.#editor;
			const configuration = {
				language: options.language,
				label: options.label,
				readonly: options.readonly,
				disabled: options.disabled,
				placeholder: options.placeholder
			};
			if (editor) untrack(() => editor.configure(configuration));
		});
	}

	mount = (container: HTMLElement) => {
		let disposed = false;

		import('./codemirror')
			.then(({ CodeMirror }) => {
				if (disposed) return;

				const fallback = container.parentElement?.querySelector('textarea');
				const focused = fallback && document.activeElement === fallback;
				const editor = new CodeMirror(container, this.#options);
				this.#editor = editor;

				if (focused) {
					editor.view.dispatch({
						selection: { anchor: fallback.selectionStart, head: fallback.selectionEnd }
					});
					editor.view.scrollDOM.scrollTop = fallback.scrollTop;
					editor.view.scrollDOM.scrollLeft = fallback.scrollLeft;
					editor.view.focus();
				}

				this.ready = true;
			})
			.catch((error: unknown) => {
				if (disposed) return;
				console.error('Could not load CodeMirror', error);
				this.failed = true;
			});

		return () => {
			disposed = true;
			this.#editor?.view.destroy();
			this.#editor = undefined;
		};
	};
}
