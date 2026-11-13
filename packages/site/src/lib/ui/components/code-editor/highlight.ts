import { rustLanguage } from '@codemirror/lang-rust';
import { typescriptLanguage } from '@codemirror/lang-javascript';
import { parseMixed } from '@lezer/common';
import { highlightTree, tagHighlighter, tags } from '@lezer/highlight';
import { diagnosticTags, diagnosticsLanguage } from './diagnostics';

export type CodeLanguage = 'rust' | 'typescript' | 'diagnostics' | 'text';

export const languages = {
	rust: rustLanguage,
	typescript: typescriptLanguage.configure({
		wrap: parseMixed((node, input) => {
			if (node.name !== 'TaggedTemplateExpression') return null;
			const tag = node.node.firstChild;
			const template = node.node.getChild('TemplateString');
			if (!tag || tag.name !== 'VariableName' || input.read(tag.from, tag.to) !== 'rs' || !template)
				return null;

			const overlay: { from: number; to: number }[] = [];
			let from = template.from + 1;
			const end = template.to - (input.read(template.to - 1, template.to) === '`' ? 1 : 0);
			for (const interpolation of template.getChildren('Interpolation')) {
				if (from < interpolation.from) overlay.push({ from, to: interpolation.from });
				from = interpolation.to;
			}
			if (from < end) overlay.push({ from, to: end });

			return overlay.length ? { parser: rustLanguage.parser, overlay } : null;
		})
	}),
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
