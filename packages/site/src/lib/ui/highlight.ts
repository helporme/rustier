import { createHighlighterCoreSync } from 'shiki/core';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';
import rust from 'shiki/langs/rust.mjs';
import typescript from 'shiki/langs/typescript.mjs';
import shellscript from 'shiki/langs/shellscript.mjs';

export const ansiColorClasses: Record<string, string> = {
	'var(--color-on-surface)': 'ansi-on-surface',
	'var(--color-on-surface-alt)': 'ansi-on-surface-alt',
	'var(--color-red)': 'ansi-red',
	'var(--color-green)': 'ansi-green',
	'var(--color-yellow)': 'ansi-yellow',
	'var(--color-blue)': 'ansi-blue',
	'var(--color-magenta)': 'ansi-magenta',
	'var(--color-cyan)': 'ansi-cyan'
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
				'terminal.ansiRed': 'var(--color-red)',
				'terminal.ansiGreen': 'var(--color-green)',
				'terminal.ansiYellow': 'var(--color-yellow)',
				'terminal.ansiBlue': 'var(--color-blue)',
				'terminal.ansiMagenta': 'var(--color-magenta)',
				'terminal.ansiCyan': 'var(--color-cyan)',
				'terminal.ansiWhite': 'var(--color-on-surface)',
				'terminal.ansiBrightBlack': 'var(--color-on-surface-alt)',
				'terminal.ansiBrightRed': 'var(--color-red)',
				'terminal.ansiBrightGreen': 'var(--color-green)',
				'terminal.ansiBrightYellow': 'var(--color-yellow)',
				'terminal.ansiBrightBlue': 'var(--color-blue)',
				'terminal.ansiBrightMagenta': 'var(--color-magenta)',
				'terminal.ansiBrightCyan': 'var(--color-cyan)',
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

export function highlight(
	value: string,
	language: 'rust' | 'typescript' | 'shellscript' | 'text' | 'ansi'
) {
	return highlighter.codeToTokensBase(value, { lang: language, theme: 'rustier' });
}

export { highlighter };
