import { EditorState, Compartment } from '@codemirror/state';
import {
	EditorView,
	drawSelection,
	highlightActiveLine,
	lineNumbers,
	keymap,
	placeholder
} from '@codemirror/view';
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import {
	bracketMatching,
	indentOnInput,
	syntaxHighlighting,
	indentUnit
} from '@codemirror/language';
import {
	autocompletion,
	completeAnyWord,
	closeBrackets,
	closeBracketsKeymap,
	completionKeymap
} from '@codemirror/autocomplete';
import { javascript } from '@codemirror/lang-javascript';
import { search, searchKeymap, highlightSelectionMatches } from '@codemirror/search';
import { highlighter, languages, type CodeLanguage } from './highlight';

const languageExtensions = {
	rust: [languages.rust, languages.rust.data.of({ autocomplete: completeAnyWord })],
	typescript: javascript({ typescript: true }),
	diagnostics: languages.diagnostics,
	text: []
};

export type EditorOptions = {
	value: string;
	language: CodeLanguage;
	label: string;
	readonly: boolean;
	disabled: boolean;
	placeholder: string;
	onchange: (value: string) => void;
};

export class CodeMirror {
	readonly view: EditorView;
	#configuration = new Compartment();

	constructor(parent: HTMLElement, options: EditorOptions) {
		this.view = new EditorView({
			parent,
			state: EditorState.create({
				doc: options.value,
				extensions: [
					history(),
					drawSelection(),
					lineNumbers(),
					highlightActiveLine(),
					highlightSelectionMatches(),
					indentOnInput(),
					bracketMatching(),
					closeBrackets(),
					autocompletion(),
					search({ top: true }),
					syntaxHighlighting(highlighter),
					keymap.of([
						...closeBracketsKeymap,
						...defaultKeymap,
						...historyKeymap,
						...searchKeymap,
						...completionKeymap,
						indentWithTab
					]),
					this.#configuration.of(this.#extensions(options)),
					EditorView.updateListener.of((update) => {
						if (update.docChanged) options.onchange(update.state.doc.toString());
					})
				]
			})
		});
	}

	#extensions(options: Omit<EditorOptions, 'value' | 'onchange'>) {
		return [
			languageExtensions[options.language],
			indentUnit.of(options.language === 'rust' ? '    ' : '  '),
			EditorState.tabSize.of(options.language === 'rust' ? 4 : 2),
			EditorState.readOnly.of(options.readonly || options.disabled),
			EditorView.editable.of(!options.readonly && !options.disabled),
			EditorView.contentAttributes.of({
				'aria-label': options.label,
				'aria-readonly': String(options.readonly || options.disabled),
				'aria-disabled': String(options.disabled),
				tabindex: options.disabled ? '-1' : '0'
			}),
			placeholder(options.placeholder)
		];
	}

	configure(options: Omit<EditorOptions, 'value' | 'onchange'>) {
		this.view.dispatch({ effects: this.#configuration.reconfigure(this.#extensions(options)) });
	}

	setValue(value: string) {
		if (this.view.state.doc.toString() === value) return;

		this.view.dispatch({
			changes: { from: 0, to: this.view.state.doc.length, insert: value }
		});
	}
}
