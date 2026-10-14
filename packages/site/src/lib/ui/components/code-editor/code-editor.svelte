<script lang="ts">
	import { onMount, tick } from 'svelte';
	import type { editor } from 'monaco-editor/editor';
	import { twMerge } from 'tailwind-merge';
	import { ansiColorClasses, highlight } from '#lib/ui/highlight';
	import type { EditorMarker, LanguageService } from './language-service';

	const browser = typeof window !== 'undefined';

	let {
		value = $bindable(),
		language,
		filename,
		colorScheme,
		readOnly = false,
		tabIndex = 0,
		extraLibs = [],
		markers = [],
		initialPreview,
		languageService,
		onNavigate,
		class: className
	}: {
		value: string;
		language: 'rust' | 'typescript' | 'shellscript' | 'text' | 'ansi';
		filename: string;
		colorScheme?:
			| 'rustier-dark'
			| 'rustier-dark-dim'
			| 'rustier-dark-dimmest'
			| 'rustier-light'
			| 'rustier-light-dim'
			| 'rustier-light-dimmest';
		readOnly?: boolean;
		tabIndex?: number;
		extraLibs?: readonly { filePath: string; content: string }[];
		markers?: readonly EditorMarker[];
		initialPreview?: {
			value: string;
			language: 'rust' | 'typescript' | 'shellscript' | 'text' | 'ansi';
			tokens: ReturnType<typeof highlight>;
		};
		languageService?: LanguageService;
		onNavigate?: () => void;
		class?: string;
	} = $props();

	let container: HTMLDivElement;
	let fallback = $state<HTMLTextAreaElement>();
	let preview = $state<HTMLPreElement>();
	let instance = $state.raw<editor.IStandaloneCodeEditor>();
	let decorations: editor.IEditorDecorationsCollection | undefined;
	let monacoAPI: typeof import('monaco-editor/editor') | undefined;
	let registerMap: typeof import('./monaco.js').registerSourceMap | undefined;
	let retainModel: typeof import('./monaco.js').retainModel | undefined;
	const models: Record<string, () => void> = {};
	const viewStates: Record<string, editor.ICodeEditorViewState | null> = {};
	let defaults:
		| typeof import('monaco-editor/languages/features/typescript/register').typescriptDefaults
		| undefined;
	let mounted = $state(false);
	let loading = $state(true);
	let failed = $state(false);
	let tokens = $derived(
		language === 'ansi'
			? highlight(value, language)
			: loading || failed
				? initialPreview?.value === value && initialPreview.language === language
					? initialPreview.tokens
					: browser
						? value.split('\n').map((content) => [{ content, color: undefined, fontStyle: 0 }])
						: highlight(value, language)
				: []
	);
	let displayValue = $derived(
		language === 'ansi'
			? tokens.map((line) => line.map((token) => token.content).join('')).join('\n')
			: value
	);
	let surface = $derived(
		colorScheme?.endsWith('-dimmest')
			? 'surface-dimest'
			: colorScheme?.endsWith('-dim')
				? 'surface-dim'
				: 'surface'
	);

	onMount(() => {
		mounted = true;

		let disposed = false;
		let cleanup: (() => void) | undefined;
		const panel = container.closest<HTMLElement>('[data-editor-pane], [role="tabpanel"]');
		const enterEditor = (event: KeyboardEvent) => {
			if (event.target !== panel || event.key !== 'Enter' || tabIndex !== -1) return;

			event.preventDefault();

			if (instance) instance.focus();
			else fallback?.focus();
		};

		panel?.addEventListener('keydown', enterEditor);

		import('./monaco.js')
			.then(
				async ({
					monaco,
					applyTheme,
					typescriptDefaults,
					registerSourceMap,
					retainModel: retain
				}) => {
					if (disposed) return;

					const styles = getComputedStyle(container);
					const fontFamily = styles.fontFamily;
					const fontSize = Number.parseFloat(styles.fontSize);
					const lineHeight = Number.parseFloat(styles.lineHeight);
					const letterSpacing = Number.parseFloat(styles.letterSpacing) || 0;
					await document.fonts.load(`${fontSize}px ${fontFamily}`);

					if (disposed) return;

					const colorScheme = matchMedia('(prefers-color-scheme: dark)');
					let palette = '';
					const updateTheme = () => {
						const styles = getComputedStyle(container);
						const next = [
							styles.colorScheme,
							colorScheme.matches,
							...[
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
							].map((name) => styles.getPropertyValue(`--color-${name}`))
						].join('|');

						if (next === palette) return;

						palette = next;
						applyTheme(container);
					};
					updateTheme();

					const uri = monaco.Uri.parse(`file:///playground/${filename}`);
					const model =
						monaco.editor.getModel(uri) ??
						monaco.editor.createModel(
							displayValue,
							language === 'text' || language === 'ansi' ? 'plaintext' : language,
							uri
						);

					models[uri.toString()] = retain(model);
					retainModel = retain;

					model.updateOptions({
						tabSize: language === 'rust' ? 4 : 2,
						bracketColorizationOptions: {
							enabled: false,
							independentColorPoolPerBracketType: false
						}
					});

					const codeEditor = monaco.editor.create(container, {
						model,
						readOnly: readOnly || language === 'ansi',
						renderValidationDecorations: readOnly || language === 'ansi' ? 'off' : 'on',
						theme: 'rustier',
						automaticLayout: true,
						ariaLabel: `${language} code editor. Press Escape then Tab or Shift+Tab to move focus out.`,
						tabIndex,
						fontFamily,
						fontSize,
						lineHeight,
						letterSpacing,
						minimap: { enabled: false },
						padding: { top: 0, bottom: 0 },
						scrollBeyondLastLine: false,
						scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8, useShadows: false },
						lineNumbers: 'off',
						lineDecorationsWidth: 8,
						glyphMargin: false,
						folding: false,
						guides: { indentation: false, bracketPairs: false, highlightActiveIndentation: false },
						renderWhitespace: 'none',
						renderLineHighlight: 'all',
						renderLineHighlightOnlyWhenFocus: true,
						overviewRulerLanes: 0,
						hideCursorInOverviewRuler: true,
						stickyScroll: { enabled: false },
						'semanticHighlighting.enabled': true,
						fixedOverflowWidgets: true,
						useShadowDOM: false
					});

					const opener = monaco.editor.registerEditorOpener({
						async openCodeEditor(_source, resource, selection) {
							if (codeEditor.getModel()?.uri.toString() !== resource.toString()) return false;

							onNavigate?.();
							await tick();
							codeEditor.layout();

							if (selection && monaco.Range.isIRange(selection)) {
								codeEditor.setSelection(selection);
								codeEditor.revealRangeInCenter(selection);
							} else if (selection) {
								codeEditor.setPosition(selection);
								codeEditor.revealPositionInCenter(selection);
							}

							codeEditor.focus();
							return true;
						}
					});

					monacoAPI = monaco;
					defaults = typescriptDefaults;
					registerMap = registerSourceMap;
					decorations = codeEditor.createDecorationsCollection();
					instance = codeEditor;

					const change = codeEditor.onDidChangeModelContent(() => {
						if (language !== 'ansi') value = codeEditor.getValue();
					});

					let escapeTab = false;
					const tabNavigation = (event: KeyboardEvent) => {
						if (!codeEditor.hasTextFocus()) return;

						if (event.key === 'Escape') {
							escapeTab = true;
						} else if (escapeTab && event.key === 'Tab') {
							escapeTab = false;
							event.stopImmediatePropagation();
						} else if (!['Shift', 'Control', 'Alt', 'Meta'].includes(event.key)) {
							escapeTab = false;
						}
					};
					const resetTabEscape = () => {
						escapeTab = false;
					};
					const blur = codeEditor.onDidBlurEditorText(resetTabEscape);

					container.addEventListener('keydown', tabNavigation, true);
					container.addEventListener('pointerdown', resetTabEscape, true);
					colorScheme.addEventListener('change', updateTheme);

					const themeObserver = new MutationObserver(updateTheme);

					for (
						let ancestor = container.parentElement;
						ancestor;
						ancestor = ancestor.parentElement
					) {
						themeObserver.observe(ancestor, {
							attributes: true,
							attributeFilter: ['class', 'style']
						});
					}

					if (fallback && document.activeElement === fallback) {
						const start = model.getPositionAt(fallback.selectionStart);
						const end = model.getPositionAt(fallback.selectionEnd);

						codeEditor.setSelection({
							startLineNumber: start.lineNumber,
							startColumn: start.column,
							endLineNumber: end.lineNumber,
							endColumn: end.column
						});
						codeEditor.focus();
					}

					loading = false;
					cleanup = () => {
						container.removeEventListener('keydown', tabNavigation, true);
						container.removeEventListener('pointerdown', resetTabEscape, true);
						colorScheme.removeEventListener('change', updateTheme);
						themeObserver.disconnect();
						blur.dispose();
						change.dispose();
						opener.dispose();

						decorations?.clear();
						decorations = undefined;
						instance = undefined;
						codeEditor.dispose();

						Object.values(models).forEach((release) => release());
					};
				}
			)
			.catch((error: unknown) => {
				if (disposed) return;

				console.error('Could not load Monaco', error);
				loading = false;
				failed = true;
			});

		return () => {
			disposed = true;
			panel?.removeEventListener('keydown', enterEditor);
			cleanup?.();
		};
	});

	$effect(() => {
		const nextValue = displayValue;
		const nextLanguage = language === 'text' || language === 'ansi' ? 'plaintext' : language;
		const nextFilename = filename;

		instance?.updateOptions({
			readOnly: readOnly || language === 'ansi',
			renderValidationDecorations: readOnly || language === 'ansi' ? 'off' : 'on',
			ariaLabel: `${language} code editor. Press Escape then Tab or Shift+Tab to move focus out.`,
			tabIndex
		});

		if (instance && monacoAPI) {
			let model = instance.getModel();
			const uri = monacoAPI.Uri.parse(`file:///playground/${nextFilename}`);

			if (model?.uri.toString() !== uri.toString()) {
				const previous = model;
				if (previous) viewStates[previous.uri.toString()] = instance.saveViewState();

				model =
					monacoAPI.editor.getModel(uri) ??
					monacoAPI.editor.createModel(nextValue, nextLanguage, uri);

				if (!models[uri.toString()] && retainModel) models[uri.toString()] = retainModel(model);

				instance.setModel(model);

				const state = viewStates[uri.toString()];
				if (state) instance.restoreViewState(state);
			} else if (model.getLanguageId() !== nextLanguage) {
				monacoAPI.editor.setModelLanguage(model, nextLanguage);
			}

			model.updateOptions({ tabSize: language === 'rust' ? 4 : 2 });

			if (model.getValue() !== nextValue) {
				model.setValue(nextValue);
			}
		}
	});

	$effect(() => {
		const diagnostics = markers;
		const model = instance?.getModel();
		const api = monacoAPI;

		if (!model || !api || language !== 'rust' || !languageService) return;

		api.editor.setModelMarkers(
			model,
			'rust-analyzer',
			diagnostics.map((marker) => {
				const start = model.getPositionAt(marker.start);
				const end = model.getPositionAt(marker.end);

				return {
					startLineNumber: start.lineNumber,
					startColumn: start.column,
					endLineNumber: end.lineNumber,
					endColumn: end.column,
					severity: marker.severity,
					message: marker.message,
					code: marker.code,
					source: 'rust-analyzer'
				};
			})
		);

		return () => {
			if (!model.isDisposed()) api.editor.setModelMarkers(model, 'rust-analyzer', []);
		};
	});

	$effect(() => {
		const codeEditor = instance;
		const api = monacoAPI;
		const service = languageService;

		if (!codeEditor || !api || !service || language !== 'rust') return;

		const hover = api.languages.registerHoverProvider('rust', {
			async provideHover(model, position, token) {
				if (
					model !== codeEditor.getModel() ||
					api.editor.getEditors().find((editor) => editor.getModel() === model) !== codeEditor
				)
					return;

				const version = model.getVersionId();
				const controller = new AbortController();
				const cancellation = token.onCancellationRequested(() => controller.abort());

				try {
					const result = await service.hover(
						model.getValue(),
						model.getOffsetAt(position),
						controller.signal
					);

					if (
						!result ||
						token.isCancellationRequested ||
						model.isDisposed() ||
						model.getVersionId() !== version
					)
						return;

					const start = model.getPositionAt(result.start);
					const end = model.getPositionAt(result.end);

					return {
						range: new api.Range(start.lineNumber, start.column, end.lineNumber, end.column),
						contents: [{ value: result.markdown, isTrusted: false }]
					};
				} finally {
					cancellation.dispose();
				}
			}
		});
		const definition = api.languages.registerDefinitionProvider('rust', {
			async provideDefinition(model, position, token) {
				if (
					model !== codeEditor.getModel() ||
					api.editor.getEditors().find((editor) => editor.getModel() === model) !== codeEditor
				)
					return;

				const version = model.getVersionId();
				const controller = new AbortController();
				const cancellation = token.onCancellationRequested(() => controller.abort());

				try {
					const result = await service.definition(
						model.getValue(),
						model.getOffsetAt(position),
						controller.signal
					);

					if (
						!result ||
						token.isCancellationRequested ||
						model.isDisposed() ||
						model.getVersionId() !== version
					)
						return;

					const start = model.getPositionAt(result.start);
					const end = model.getPositionAt(result.end);

					return {
						uri: model.uri,
						range: new api.Range(start.lineNumber, start.column, end.lineNumber, end.column)
					};
				} finally {
					cancellation.dispose();
				}
			}
		});

		return () => {
			hover.dispose();
			definition.dispose();
		};
	});

	$effect(() => {
		const lines = tokens;

		if (!instance || !decorations) return;

		const ranges: editor.IModelDeltaDecoration[] = [];

		if (language === 'ansi') {
			for (const [index, line] of lines.entries()) {
				let column = 1;

				for (const token of line) {
					const start = column;
					column += token.content.length;

					if (!token.content) continue;

					const fontStyle = token.fontStyle ?? 0;
					const classes = [
						ansiColorClasses[token.color ?? ''] ?? '',
						fontStyle & 1 ? 'ansi-italic' : '',
						fontStyle & 2 ? 'ansi-bold' : '',
						fontStyle & 4 ? 'ansi-underline' : ''
					]
						.filter(Boolean)
						.join(' ');

					if (classes) {
						ranges.push({
							range: {
								startLineNumber: index + 1,
								startColumn: start,
								endLineNumber: index + 1,
								endColumn: column
							},
							options: { inlineClassName: classes }
						});
					}
				}
			}
		}

		decorations.set(ranges);
	});

	$effect(() => {
		const libraries = extraLibs;
		const languageDefaults = defaults;
		const registerSourceMap = registerMap;

		if (!instance || !languageDefaults || !registerSourceMap) return;

		const disposables = libraries.map(({ content, filePath }) =>
			filePath.endsWith('.map')
				? registerSourceMap(content, filePath)
				: languageDefaults.addExtraLib(content, filePath)
		);

		return () => disposables.forEach((disposable) => disposable.dispose());
	});
</script>

<div
	class={twMerge(
		'code-editor relative h-full min-h-0 w-full bg-surface text-on-surface',
		'[&_.monaco-editor-overlaymessage]:animate-none! [&_.monaco-editor-overlaymessage_.anchor]:hidden! [&_.monaco-editor-overlaymessage_.message]:rounded-none! [&_.monaco-editor-overlaymessage_.message]:border-0! [&_.monaco-editor-overlaymessage_.message]:bg-surface-bright! [&_.monaco-editor-overlaymessage_.message]:text-on-surface! [&_.monaco-editor-overlaymessage_.message]:shadow-none!',
		'[&_.cursors-layer_.cursor]:border-on-surface! [&_.cursors-layer_.cursor]:bg-on-surface! [&_.cursors-layer_.cursor]:text-surface!',
		'[&_.cursors-layer:is(.cursor-block-outline-style,.cursor-underline-style,.cursor-underline-thin-style)_.cursor]:bg-transparent!',
		'[&_.suggest-widget_.details-label]:text-on-surface-alt! [&_.suggest-widget_.monaco-highlighted-label_.highlight]:text-primary! [&_.suggest-widget_.monaco-icon-label]:text-on-surface! [&_.suggest-widget_.monaco-list-row]:bg-transparent! [&_.suggest-widget_.monaco-list-row]:text-on-surface! [&_.suggest-widget_.monaco-list-row.focused]:bg-surface-dim! [&_.suggest-widget_.monaco-list-row.selected]:bg-surface-dim!',
		'[&_.monaco-scrollable-element_.shadow]:hidden! [&_.scroll-decoration]:hidden!',
		'[&_.ansi-blue]:text-blue! [&_.ansi-bold]:font-bold! [&_.ansi-cyan]:text-cyan! [&_.ansi-green]:text-green! [&_.ansi-italic]:italic! [&_.ansi-magenta]:text-magenta! [&_.ansi-on-surface]:text-on-surface! [&_.ansi-on-surface-alt]:text-on-surface-alt! [&_.ansi-red]:text-red! [&_.ansi-underline]:underline! [&_.ansi-yellow]:text-yellow!',
		'[&_:is(.monaco-hover,.monaco-resizable-hover,.suggest-widget,.suggest-details,.parameter-hints-widget)]:bg-surface-bright! [&_:is(.monaco-hover,.monaco-resizable-hover,.suggest-widget,.suggest-details,.parameter-hints-widget)]:text-on-surface!',
		'[&_:is(.monaco-hover,.monaco-resizable-hover,.suggest-widget,.suggest-details,.parameter-hints-widget,.monaco-editor,.monaco-component)]:[color-scheme:inherit]!',
		'[&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorWidget-background:var(--color-surface-bright)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorWidget-border:transparent] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorWidget-foreground:var(--color-on-surface)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-widget-shadow:transparent]',
		'[&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorSuggestWidget-background:var(--color-surface-bright)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorSuggestWidget-border:transparent] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorSuggestWidget-focusHighlightForeground:var(--color-primary)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorSuggestWidget-foreground:var(--color-on-surface)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorSuggestWidget-highlightForeground:var(--color-primary)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorSuggestWidget-selectedBackground:var(--color-surface-dim)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorSuggestWidget-selectedForeground:var(--color-on-surface)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorSuggestWidget-selectedIconForeground:var(--color-blue)]',
		'[&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorHoverWidget-background:var(--color-surface-bright)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorHoverWidget-border:transparent] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorHoverWidget-foreground:var(--color-on-surface)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-editorHoverWidget-statusBarBackground:var(--color-surface-dim)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-textLink-activeForeground:var(--color-primary)] [&_:is(.monaco-editor,.monaco-component)]:[--vscode-textLink-foreground:var(--color-primary)]',
		'[&_.hover-copy-button]:rounded-none! [&_.hover-row]:rounded-none! [&_:is(.monaco-hover,.monaco-resizable-hover,.suggest-widget,.suggest-details,.parameter-hints-widget)]:rounded-none! [&_:is(.monaco-hover,.monaco-resizable-hover,.suggest-widget,.suggest-details,.parameter-hints-widget)]:border-0! [&_:is(.monaco-hover,.monaco-resizable-hover,.suggest-widget,.suggest-details,.parameter-hints-widget)]:shadow-none!',
		className
	)}
	style:color-scheme={colorScheme
		? colorScheme.startsWith('rustier-dark')
			? 'dark'
			: 'light'
		: undefined}
	style:--editor-surface={`var(--editor-background, var(--color-${surface}))`}
	style:background-color={colorScheme ? 'var(--editor-surface)' : undefined}
>
	<div bind:this={container} class="h-full w-full"></div>
	{#if loading || failed}
		<div class="absolute inset-0 flex flex-col bg-inherit">
			{#if failed}
				<p class="py-3 pl-2 text-on-surface-alt" role="alert">
					editor unavailable — highlighted text mode
				</p>
			{/if}
			<div class="relative min-h-0 flex-1">
				<pre
					bind:this={preview}
					aria-hidden={mounted}
					aria-label={`${language} code editor`}
					class="h-full overflow-hidden pl-2"
					style:tab-size={language === 'rust'
						? 4
						: 2}>{#each tokens as line, index (index)}{#each line as token, tokenIndex (tokenIndex)}<span
								class:italic={!!((token.fontStyle ?? 0) & 1)}
								class:font-bold={!!((token.fontStyle ?? 0) & 2)}
								class:underline={!!((token.fontStyle ?? 0) & 4)}
								style:color={token.color}>{token.content}</span
							>{/each}{index < tokens.length - 1 ? '\n' : ''}{/each}{language !== 'ansi' &&
					value.endsWith('\n')
						? '\n'
						: ''}</pre>
				{#if mounted}
					<textarea
						bind:this={fallback}
						value={displayValue}
						oninput={(event) => {
							if (language !== 'ansi') value = event.currentTarget.value;
						}}
						readonly={readOnly || language === 'ansi'}
						aria-label={`${language} code editor`}
						tabindex={tabIndex}
						spellcheck="false"
						wrap="off"
						onscroll={(event) => {
							if (!preview) return;

							preview.scrollTop = event.currentTarget.scrollTop;
							preview.scrollLeft = event.currentTarget.scrollLeft;
						}}
						style:tab-size={language === 'rust' ? 4 : 2}
						class="absolute inset-0 h-full w-full resize-none bg-transparent pl-2 text-transparent caret-on-surface outline-none selection:bg-(--editor-selection) selection:text-on-surface"
					></textarea>
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.code-editor {
		--editor-selection: light-dark(
			hsl(from var(--editor-surface, var(--color-surface)) h s calc(l - 5)),
			hsl(from var(--editor-surface, var(--color-surface)) h s calc(l + 15))
		);
	}

	.code-editor :global(.monaco-editor .mtk1) {
		color: var(--color-on-surface) !important;
	}

	.code-editor :global(.monaco-editor .mtk2) {
		color: var(--color-on-surface-alt) !important;
	}

	.code-editor :global(.monaco-editor .mtk3) {
		color: var(--color-red) !important;
	}

	.code-editor :global(.monaco-editor .mtk4) {
		color: var(--color-green) !important;
	}

	.code-editor :global(.monaco-editor .mtk5) {
		color: var(--color-yellow) !important;
	}

	.code-editor :global(.monaco-editor .mtk6) {
		color: var(--color-blue) !important;
	}

	.code-editor :global(.monaco-editor .mtk7) {
		color: var(--color-magenta) !important;
	}

	.code-editor ::selection {
		background-color: var(--editor-selection);
	}

	.code-editor :global(.monaco-menu-container) {
		--vscode-menu-background: var(--color-surface-bright);
		--vscode-menu-foreground: var(--color-on-surface);
		--vscode-menu-selectionBackground: var(--color-selection);
		--vscode-menu-selectionForeground: var(--color-on-surface);
		--vscode-menu-separatorBackground: transparent;
		--vscode-menu-border: transparent;

		border-radius: 0 !important;
		box-shadow: none !important;
		font: inherit !important;
		letter-spacing: inherit !important;
	}

	.code-editor :global(.monaco-menu-container *) {
		font-family: inherit;
		font-size: inherit !important;
		line-height: inherit !important;
		letter-spacing: inherit;
	}

	.code-editor :global(.monaco-menu-container .monaco-scrollable-element) {
		border-radius: 0 !important;
		box-shadow: none !important;
		animation: none !important;
	}

	.code-editor :global(.monaco-menu .monaco-action-bar.vertical) {
		padding: 0 !important;
	}

	.code-editor :global(.monaco-menu .monaco-action-bar.vertical .action-menu-item) {
		height: auto !important;
		margin: 0 !important;
		border-radius: 0 !important;
	}

	.code-editor :global(.monaco-menu .monaco-action-bar.vertical .action-label:not(.separator)) {
		padding: 0 0 0 1ch !important;
	}

	.code-editor :global(.monaco-menu .monaco-action-bar.vertical .keybinding) {
		padding: 0 1ch 0 4ch !important;
		color: var(--color-on-surface-alt);
		opacity: 1 !important;
	}

	.code-editor :global(.monaco-menu .monaco-action-bar.vertical .menu-item-check) {
		display: none !important;
	}

	.code-editor :global(.monaco-menu .monaco-action-bar.vertical .submenu-indicator) {
		padding: 0 1ch 0 4ch !important;
		font: inherit !important;
		height: auto !important;
	}

	.code-editor :global(.monaco-menu .monaco-action-bar.vertical .submenu-indicator::before) {
		content: '>' !important;
		margin: 0 !important;
		font: inherit !important;
	}

	.code-editor :global(.monaco-menu .monaco-action-bar.vertical .action-label.separator) {
		display: block !important;
		width: 0;
		min-width: 100%;
		height: auto !important;
		margin: 0 !important;
		padding: 0 1ch !important;
		border: 0 !important;
		overflow: hidden;
		white-space: nowrap;
		color: var(--color-on-surface-alt);
	}

	.code-editor :global(.monaco-menu .monaco-action-bar.vertical .action-label.separator::before) {
		content: '----------------------------------------------------------------';
	}

	.code-editor
		:global(
			.monaco-menu .monaco-action-bar.vertical .action-item.disabled .action-label:not(.separator)
		) {
		color: var(--color-on-surface-alt);
	}

	.code-editor :global(.monaco-editor .margin-view-overlays .current-line) {
		background-color: transparent !important;
	}

	.code-editor :global(.monaco-editor .selected-text),
	.code-editor :global(.monaco-editor.focused .view-overlays .current-line),
	.code-editor :global(.monaco-editor.focused .margin-view-overlays .current-line-margin-both) {
		background-color: var(--editor-selection) !important;
	}
</style>
