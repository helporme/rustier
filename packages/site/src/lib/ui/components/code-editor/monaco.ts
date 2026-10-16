import * as monaco from 'monaco-editor/editor';
import 'monaco-editor/languages/definitions/rust/register';
import 'monaco-editor/languages/definitions/typescript/register';
import 'monaco-editor/editor/contrib/clipboard/browser/clipboard';
import 'monaco-editor/editor/contrib/find/browser/findController';
import 'monaco-editor/editor/contrib/comment/browser/comment';
import 'monaco-editor/editor/contrib/bracketMatching/browser/bracketMatching';
import 'monaco-editor/editor/contrib/hover/browser/hoverContribution';
import 'monaco-editor/editor/contrib/suggest/browser/suggestController';
import 'monaco-editor/editor/contrib/gotoSymbol/browser/goToCommands';
import 'monaco-editor/editor/contrib/gotoSymbol/browser/link/goToDefinitionAtPosition';
import {
	ModuleKind,
	ModuleResolutionKind,
	ScriptTarget,
	getTypeScriptWorker,
	typescriptDefaults
} from 'monaco-editor/languages/features/typescript/register';
import 'monaco-editor/languages/features/typescript/tsMode';
import EditorWorker from 'monaco-editor/editor/editor.worker?worker';
import TypeScriptWorker from 'monaco-editor/language/typescript/ts.worker?worker';
import { textmateThemeToMonacoTheme } from '@shikijs/monaco';
import { TraceMap, originalPositionFor } from '@jridgewell/trace-mapping';
import { highlighter, tokenColors } from '#lib/ui/highlight';
import { registerTokenization, tokenModifiers } from './tokenization';

const sourceMaps = new Map<string, TraceMap>();
const modelReferences = new Map<monaco.editor.ITextModel, number>();
const libraries = new Map<
	string,
	{ content: string; references: number; registration: monaco.IDisposable }
>();

export function retainLibrary(content: string, filePath: string): monaco.IDisposable {
	let library = libraries.get(filePath);

	if (!library || library.content !== content) {
		library = {
			content,
			references: 0,
			registration: filePath.endsWith('.map')
				? registerSourceMap(content, filePath)
				: typescriptDefaults.addExtraLib(content, filePath)
		};
		libraries.set(filePath, library);
	}

	const retained = library;
	retained.references++;

	if (!filePath.endsWith('.map')) {
		const model = monaco.editor.getModel(monaco.Uri.parse(filePath));

		if (model && model.getValue() !== content) model.setValue(content);
	}

	return {
		dispose() {
			if (--retained.references !== 0) return;

			retained.registration.dispose();

			if (libraries.get(filePath) === retained) libraries.delete(filePath);
		}
	};
}

export function retainModel(model: monaco.editor.ITextModel) {
	modelReferences.set(model, (modelReferences.get(model) ?? 0) + 1);

	return () => {
		const references = (modelReferences.get(model) ?? 1) - 1;

		if (references > 0) modelReferences.set(model, references);
		else {
			modelReferences.delete(model);
			if (!model.isDisposed()) model.dispose();
		}
	};
}

self.MonacoEnvironment = {
	getWorker(_moduleId, label) {
		return label === 'typescript' || label === 'javascript'
			? new TypeScriptWorker()
			: new EditorWorker();
	}
};

typescriptDefaults.setDiagnosticsOptions({
	noSemanticValidation: false,
	noSyntaxValidation: false,
	noSuggestionDiagnostics: true
});

typescriptDefaults.setCompilerOptions({
	...typescriptDefaults.getCompilerOptions(),
	strict: true,
	target: ScriptTarget.ESNext,
	module: ModuleKind.ESNext,
	moduleResolution: ModuleResolutionKind.NodeJs
});

typescriptDefaults.setModeConfiguration({
	...typescriptDefaults.modeConfiguration,
	definitions: false
});

monaco.languages.registerDefinitionProvider('typescript', {
	async provideDefinition(model, position, token) {
		const accessor = await getTypeScriptWorker();
		const worker = await accessor(model.uri);
		const definitions = await worker.getDefinitionAtPosition(
			model.uri.toString(),
			model.getOffsetAt(position)
		);

		if (!definitions || model.isDisposed() || token.isCancellationRequested) return [];

		const locations: monaco.languages.Location[] = [];

		for (const definition of definitions) {
			const uri = monaco.Uri.parse(definition.fileName);
			let target = monaco.editor.getModel(uri);
			let content: string | undefined =
				target?.getValue() ?? typescriptDefaults.getExtraLibs()[definition.fileName]?.content;

			if (content === undefined) {
				content = await worker.getScriptText(definition.fileName);
			}

			if (content === undefined && uri.path.startsWith('/lib.')) {
				content = await worker.getScriptText(uri.path.slice(1));
			}

			if (content === undefined || model.isDisposed() || token.isCancellationRequested) continue;

			const map = sourceMaps.get(uri.toString());

			if (map) {
				const lines = content.slice(0, definition.textSpan.start).split('\n');
				const original = originalPositionFor(map, {
					line: lines.length,
					column: lines[lines.length - 1].length
				});

				if (original.source !== null && original.line !== null && original.column !== null) {
					const source = monaco.Uri.parse(original.source);
					const sourceModel = monaco.editor.getModel(source);

					if (sourceModel) {
						const word = sourceModel.getWordAtPosition({
							lineNumber: original.line,
							column: original.column + 1
						});

						locations.push({
							uri: source,
							range: new monaco.Range(
								original.line,
								word?.startColumn ?? original.column + 1,
								original.line,
								word?.endColumn ?? original.column + 1
							)
						});
						continue;
					}
				}
			}

			target ??= monaco.editor.createModel(content, 'typescript', uri);

			const start = target.getPositionAt(definition.textSpan.start);
			const end = target.getPositionAt(definition.textSpan.start + definition.textSpan.length);

			locations.push({
				uri,
				range: new monaco.Range(start.lineNumber, start.column, end.lineNumber, end.column)
			});
		}

		return locations;
	}
});

monaco.languages.register({ id: 'shellscript', extensions: ['.sh'], aliases: ['Shell'] });

registerTokenization(monaco);

export function registerSourceMap(content: string, filePath: string) {
	const resource = filePath.slice(0, -'.map'.length);
	const map = new TraceMap(content, filePath);

	sourceMaps.set(resource, map);

	return {
		dispose() {
			if (sourceMaps.get(resource) === map) sourceMaps.delete(resource);
		}
	};
}

export async function transpile(source: string, filename: string) {
	const uri = monaco.Uri.parse(`file:///playground/${filename}`);
	const existing = monaco.editor.getModel(uri);
	const model = existing ?? monaco.editor.createModel(source, 'typescript', uri);
	const release = retainModel(model);

	try {
		if (model.getValue() !== source) model.setValue(source);

		const accessor = await getTypeScriptWorker();
		const worker = await accessor(model.uri);
		const output = await worker.getEmitOutput(model.uri.toString());
		const script = output.outputFiles.find((file) => file.name.endsWith('.js'));

		if (!script) throw new Error(`TypeScript produced no JavaScript for ${filename}`);

		return script.text;
	} finally {
		release();
	}
}

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
			...tokenColors.flatMap((color) =>
				Array.from({ length: 16 }, (_, mask) => {
					const modifiers = tokenModifiers.filter((_, index) => mask & (1 << index));

					return {
						token: [`rustier-${color}`, ...modifiers].join('.'),
						foreground: colors[color].slice(1),
						fontStyle: modifiers.join(' ')
					};
				})
			),
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
			'editorWidget.border': '#00000000',
			'editorSuggestWidget.background': colors.surface,
			'editorSuggestWidget.foreground': colors['on-surface'],
			'editorSuggestWidget.selectedBackground': colors.selection,
			'editorSuggestWidget.selectedForeground': colors['on-surface'],
			'editorSuggestWidget.selectedIconForeground': colors['on-surface'],
			'editorSuggestWidget.highlightForeground': colors.blue,
			'editorSuggestWidget.focusHighlightForeground': colors.blue,
			'editorHoverWidget.background': colors.surface,
			'editorHoverWidget.foreground': colors['on-surface'],
			'editorHoverWidget.border': '#00000000',
			'editorSuggestWidget.border': '#00000000',
			'widget.shadow': '#00000000',
			'scrollbar.shadow': '#00000000',
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

export { monaco, typescriptDefaults };
