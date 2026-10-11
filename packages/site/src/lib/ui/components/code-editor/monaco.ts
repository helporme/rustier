import * as monaco from 'monaco-editor/editor';
import 'monaco-editor/languages/definitions/rust/register';
import 'monaco-editor/languages/definitions/typescript/register';
import 'monaco-editor/editor/contrib/clipboard/browser/clipboard';
import 'monaco-editor/editor/contrib/find/browser/findController';
import 'monaco-editor/editor/contrib/comment/browser/comment';
import 'monaco-editor/editor/contrib/bracketMatching/browser/bracketMatching';
import 'monaco-editor/editor/contrib/hover/browser/hoverContribution';
import 'monaco-editor/editor/contrib/suggest/browser/suggestController';
import { typescriptDefaults } from 'monaco-editor/languages/features/typescript/register';
import EditorWorker from 'monaco-editor/editor/editor.worker?worker';
import TypeScriptWorker from 'monaco-editor/language/typescript/ts.worker?worker';
import { shikiToMonaco, textmateThemeToMonacoTheme } from '@shikijs/monaco';
import { highlighter } from '#lib/ui/highlight';

self.MonacoEnvironment = {
	getWorker(_moduleId, label) {
		return label === 'typescript' || label === 'javascript'
			? new TypeScriptWorker()
			: new EditorWorker();
	}
};

typescriptDefaults.setDiagnosticsOptions({
	noSemanticValidation: true,
	noSuggestionDiagnostics: true
});

monaco.languages.register({ id: 'shellscript', extensions: ['.sh'], aliases: ['Shell'] });

shikiToMonaco(highlighter, monaco as unknown as Parameters<typeof shikiToMonaco>[1]);

export function applyTheme(container: HTMLElement) {
	const scheme = getComputedStyle(container).colorScheme;
	const dark =
		scheme === 'dark' || (scheme !== 'light' && matchMedia('(prefers-color-scheme: dark)').matches);

	const palette = document.createElement('canvas');
	palette.width = 1;
	palette.height = 1;
	palette.hidden = true;

	const context = palette.getContext('2d');

	if (!context) {
		throw new Error('Could not resolve the editor palette');
	}

	container.append(palette);

	const colors = Object.fromEntries(
		[
			'surface',
			'surface-bright',
			'selection',
			'on-surface',
			'on-surface-alt',
			'red',
			'green',
			'yellow',
			'blue',
			'magenta'
		].map((name) => {
			palette.style.color = `var(--color-${name})`;
			context.fillStyle = getComputedStyle(palette).color;
			context.fillRect(0, 0, 1, 1);

			const channels = context.getImageData(0, 0, 1, 1).data.slice(0, 3);
			const color = Array.from(channels, (channel) => channel.toString(16).padStart(2, '0')).join(
				''
			);

			return [name, `#${color}`];
		})
	);

	palette.remove();

	const textmateTheme = highlighter.getTheme('rustier');
	const syntaxTheme = textmateThemeToMonacoTheme(textmateTheme);

	monaco.editor.defineTheme('rustier', {
		base: dark ? 'vs-dark' : 'vs',
		inherit: false,
		encodedTokensColors: [
			colors['on-surface'],
			colors['on-surface-alt'],
			colors.red,
			colors.green,
			colors.yellow,
			colors.blue,
			colors.magenta
		],
		rules: [
			{ token: '', foreground: colors['on-surface'].slice(1) },
			...syntaxTheme.rules.map((rule) => {
				const color = textmateTheme.colorReplacements?.[`#${rule.foreground}`];

				return {
					...rule,
					background: undefined,
					foreground:
						typeof color === 'string'
							? colors[color.slice('var(--color-'.length, -1)].slice(1)
							: rule.foreground
				};
			})
		],
		colors: {
			'editor.background': '#00000000',
			'editor.foreground': colors['on-surface'],
			'editorLineNumber.foreground': colors['on-surface-alt'],
			'editorLineNumber.activeForeground': colors['on-surface'],
			'editorCursor.foreground': colors['on-surface'],
			'editor.selectionBackground': colors.selection,
			'editor.inactiveSelectionBackground': colors.selection,
			'editor.lineHighlightBackground': colors.selection,
			'editor.lineHighlightBorder': '#00000000',
			'editorWidget.background': colors.surface,
			'editorWidget.foreground': colors['on-surface'],
			'editorWidget.border': colors['surface-bright'],
			'editorSuggestWidget.background': colors.surface,
			'editorSuggestWidget.foreground': colors['on-surface'],
			'editorSuggestWidget.selectedBackground': colors.selection,
			'editorSuggestWidget.selectedForeground': colors['on-surface'],
			'editorSuggestWidget.selectedIconForeground': colors['on-surface'],
			'editorSuggestWidget.highlightForeground': colors.blue,
			'editorSuggestWidget.focusHighlightForeground': colors.blue,
			'editorHoverWidget.background': colors.surface,
			'editorHoverWidget.foreground': colors['on-surface'],
			'editorHoverWidget.border': colors['surface-bright'],
			'editorGutter.background': '#00000000',
			'editorError.foreground': colors.red,
			'editorWarning.foreground': colors.yellow,
			'editorInfo.foreground': colors.blue,
			focusBorder: '#00000000',
			'scrollbarSlider.background': `${colors['on-surface-alt']}40`,
			'scrollbarSlider.hoverBackground': `${colors['on-surface-alt']}70`,
			'scrollbarSlider.activeBackground': `${colors['on-surface-alt']}90`
		}
	});

	monaco.editor.setTheme('rustier');
}

export { monaco };
