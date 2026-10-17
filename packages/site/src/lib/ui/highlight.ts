import { createHighlighterCoreSync } from 'shiki/core';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';
import rust from 'shiki/langs/rust.mjs';
import typescript from 'shiki/langs/typescript.mjs';
import shellscript from 'shiki/langs/shellscript.mjs';

export const tokenColors = [
	'on-surface',
	'on-surface-alt',
	'on-red-surface',
	'on-green-surface',
	'on-yellow-surface',
	'on-blue-surface',
	'on-magenta-surface'
] as const;

export const ansiColorClasses: Record<string, string> = {
	'var(--color-on-surface)': 'ansi-on-surface',
	'var(--color-on-surface-alt)': 'ansi-on-surface-alt',
	'var(--color-on-red-surface)': 'ansi-red',
	'var(--color-on-green-surface)': 'ansi-green',
	'var(--color-on-yellow-surface)': 'ansi-yellow',
	'var(--color-on-blue-surface)': 'ansi-blue',
	'var(--color-on-magenta-surface)': 'ansi-magenta',
	'var(--color-on-cyan-surface)': 'ansi-cyan'
};

const highlighter = createHighlighterCoreSync({
	engine: createJavaScriptRegexEngine(),
	langs: [rust, typescript, shellscript],
	themes: [
		{
			name: 'rustier',
			fg: 'var(--color-on-surface)',
			bg: 'var(--color-surface)',
			colors: {
				'terminal.ansiBlack': 'var(--color-on-surface)',
				'terminal.ansiRed': 'var(--color-on-red-surface)',
				'terminal.ansiGreen': 'var(--color-on-green-surface)',
				'terminal.ansiYellow': 'var(--color-on-yellow-surface)',
				'terminal.ansiBlue': 'var(--color-on-blue-surface)',
				'terminal.ansiMagenta': 'var(--color-on-magenta-surface)',
				'terminal.ansiCyan': 'var(--color-on-cyan-surface)',
				'terminal.ansiWhite': 'var(--color-on-surface)',
				'terminal.ansiBrightBlack': 'var(--color-on-surface-alt)',
				'terminal.ansiBrightRed': 'var(--color-on-red-surface)',
				'terminal.ansiBrightGreen': 'var(--color-on-green-surface)',
				'terminal.ansiBrightYellow': 'var(--color-on-yellow-surface)',
				'terminal.ansiBrightBlue': 'var(--color-on-blue-surface)',
				'terminal.ansiBrightMagenta': 'var(--color-on-magenta-surface)',
				'terminal.ansiBrightCyan': 'var(--color-on-cyan-surface)',
				'terminal.ansiBrightWhite': 'var(--color-on-surface)'
			},
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
					settings: { foreground: 'var(--color-on-magenta-surface)' }
				},
				{
					scope: ['keyword.operator'],
					settings: { foreground: 'var(--color-on-surface)' }
				},
				{
					scope: ['string'],
					settings: { foreground: 'var(--color-on-green-surface)' }
				},
				{
					scope: ['constant.numeric', 'constant.character.escape'],
					settings: { foreground: 'var(--color-on-yellow-surface)' }
				}
			]
		}
	]
});

export function highlight(
	value: string,
	language: 'rust' | 'typescript' | 'shellscript' | 'text' | 'ansi'
) {
	return highlighter.codeToTokensBase(value, { lang: language, theme: 'rustier' });
}

export { highlighter };
