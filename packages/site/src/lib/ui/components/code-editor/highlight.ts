import { rustLanguage } from '@codemirror/lang-rust';
import { typescriptLanguage } from '@codemirror/lang-javascript';
import { highlightTree, tagHighlighter, tags } from '@lezer/highlight';
import { diagnosticTags, diagnosticsLanguage } from './diagnostics';

export type CodeLanguage = 'rust' | 'typescript' | 'diagnostics' | 'text';

export const languages = {
	rust: rustLanguage,
	typescript: typescriptLanguage,
	diagnostics: diagnosticsLanguage
};

// Keep token colors when the page-wide ::selection rule would otherwise recolor code.
export const highlighter = tagHighlighter([
	{
		tag: [tags.keyword, tags.modifier, tags.self, diagnosticTags.code],
		class: 'text-editor-keyword selection:text-editor-keyword!'
	},
	{
		tag: [tags.typeName, tags.className, tags.namespace],
		class: 'text-editor-type selection:text-editor-type!'
	},
	{ tag: [tags.string, tags.character], class: 'text-editor-string selection:text-editor-string!' },
	{
		tag: [tags.number, tags.bool, tags.null],
		class: 'text-editor-number selection:text-editor-number!'
	},
	{
		tag: [tags.function(tags.variableName), tags.function(tags.propertyName)],
		class: 'text-editor-function selection:text-editor-function!'
	},
	{ tag: tags.comment, class: 'text-editor-comment italic selection:text-editor-comment!' },
	{
		tag: diagnosticTags.warning,
		class: 'text-editor-warning selection:text-editor-warning!'
	},
	{
		tag: diagnosticTags.error,
		class: 'text-editor-error selection:text-editor-error!'
	},
	{
		tag: diagnosticTags.location,
		class: 'text-editor-comment selection:text-editor-comment!'
	}
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
