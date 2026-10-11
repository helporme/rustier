import { createHighlighterCoreSync } from 'shiki/core';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';
import rust from 'shiki/langs/rust.mjs';
import typescript from 'shiki/langs/typescript.mjs';
import shellscript from 'shiki/langs/shellscript.mjs';

const highlighter = createHighlighterCoreSync({
	engine: createJavaScriptRegexEngine(),
	langs: [rust, typescript, shellscript],
	themes: [
		{
			name: 'rustier',
			fg: 'var(--color-on-surface)',
			bg: 'var(--color-surface)',
			settings: [
				{
					scope: ['comment'],
					settings: { foreground: 'var(--color-on-surface-alt)' }
				},
				{
					scope: [
						'keyword',
						'storage.type',
						'storage.modifier',
						'entity.name.function.macro',
						'entity.name.type.primitive',
						'entity.name.type.numeric'
					],
					settings: { foreground: 'var(--color-magenta)' }
				},
				{
					scope: ['keyword.operator'],
					settings: { foreground: 'var(--color-on-surface)' }
				},
				{
					scope: ['string'],
					settings: { foreground: 'var(--color-green)' }
				},
				{
					scope: ['constant.numeric', 'constant.character.escape'],
					settings: { foreground: 'var(--color-yellow)' }
				}
			]
		}
	]
});

export function highlight(value: string, language: 'rust' | 'typescript' | 'shellscript' | 'text') {
	return highlighter.codeToTokensBase(value, { lang: language, theme: 'rustier' });
}

export { highlighter };
