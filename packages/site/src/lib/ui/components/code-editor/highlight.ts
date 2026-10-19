import { rustLanguage } from '@codemirror/lang-rust';
import { typescriptLanguage } from '@codemirror/lang-javascript';
import { highlightTree, tagHighlighter, tags } from '@lezer/highlight';

export type CodeLanguage = 'rust' | 'typescript' | 'text';

export const languages = { rust: rustLanguage, typescript: typescriptLanguage };

export const highlighter = tagHighlighter([
	{ tag: [tags.keyword, tags.modifier, tags.self, tags.typeName], class: 'text-editor-keyword' },
	{ tag: [tags.string, tags.character, tags.number, tags.bool], class: 'text-editor-string' },
	{
		tag: [tags.function(tags.variableName), tags.function(tags.propertyName)],
		class: 'text-editor-function'
	},
	{ tag: tags.comment, class: 'text-editor-comment italic' }
]);

export function highlight(source: string, language: CodeLanguage) {
	if (language === 'text') return [{ from: 0, text: source, className: undefined }];

	const tokens: { from: number; text: string; className?: string }[] = [];
	let position = 0;

	highlightTree(languages[language].parser.parse(source), highlighter, (from, to, className) => {
		if (from > position) tokens.push({ from: position, text: source.slice(position, from) });
		tokens.push({ from, text: source.slice(from, to), className });
		position = to;
	});

	if (position < source.length) tokens.push({ from: position, text: source.slice(position) });
	return tokens;
}
