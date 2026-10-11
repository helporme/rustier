<script lang="ts">
	import { onMount } from 'svelte';
	import type { editor } from 'monaco-editor/editor';
	import { twMerge } from 'tailwind-merge';
	import { highlight } from '#lib/ui/highlight';

	let {
		value = $bindable(),
		language,
		filename,
		colorScheme,
		readOnly = false,
		class: className
	}: {
		value: string;
		language: 'rust' | 'typescript' | 'shellscript' | 'text';
		filename: string;
		colorScheme?:
			| 'rustier-dark'
			| 'rustier-dark-dim'
			| 'rustier-dark-dimmest'
			| 'rustier-light'
			| 'rustier-light-dim'
			| 'rustier-light-dimmest';
		readOnly?: boolean;
		class?: string;
	} = $props();

	let container: HTMLDivElement;
	let fallback = $state<HTMLTextAreaElement>();
	let preview = $state<HTMLPreElement>();
	let instance: editor.IStandaloneCodeEditor | undefined;
	let mounted = $state(false);
	let loading = $state(true);
	let failed = $state(false);
	let tokens = $derived(loading || failed ? highlight(value, language) : []);
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

		import('./monaco.js')
			.then(async ({ monaco, applyTheme }) => {
				if (disposed) return;

				const styles = getComputedStyle(container);
				const fontFamily = styles.fontFamily;
				const fontSize = Number.parseFloat(styles.fontSize);
				const lineHeight = Number.parseFloat(styles.lineHeight);
				const letterSpacing = Number.parseFloat(styles.letterSpacing) || 0;
				await document.fonts.load(`${fontSize}px ${fontFamily}`);

				if (disposed) return;

				const colorScheme = matchMedia('(prefers-color-scheme: dark)');
				const updateTheme = () => applyTheme(container);
				updateTheme();

				const model = monaco.editor.createModel(
					value,
					language === 'text' ? 'plaintext' : language,
					monaco.Uri.parse(`file:///playground/${filename}`)
				);

				model.updateOptions({
					tabSize: language === 'rust' ? 4 : 2,
					bracketColorizationOptions: {
						enabled: false,
						independentColorPoolPerBracketType: false
					}
				});

				const codeEditor = monaco.editor.create(container, {
					model,
					readOnly,
					theme: 'rustier',
					automaticLayout: true,
					ariaLabel: `${language} code editor`,
					fontFamily,
					fontSize,
					lineHeight,
					letterSpacing,
					minimap: { enabled: false },
					padding: { top: 0, bottom: 0 },
					scrollBeyondLastLine: false,
					scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
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
					fixedOverflowWidgets: true
				});

				instance = codeEditor;

				const change = codeEditor.onDidChangeModelContent(() => {
					value = model.getValue();
				});

				colorScheme.addEventListener('change', updateTheme);

				const themeObserver = new MutationObserver(updateTheme);

				for (let ancestor = container.parentElement; ancestor; ancestor = ancestor.parentElement) {
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
					colorScheme.removeEventListener('change', updateTheme);
					themeObserver.disconnect();
					change.dispose();
					codeEditor.dispose();
					instance = undefined;
					model.dispose();
				};
			})
			.catch((error: unknown) => {
				if (disposed) return;

				console.error('Could not load Monaco', error);
				loading = false;
				failed = true;
			});

		return () => {
			disposed = true;
			cleanup?.();
		};
	});

	$effect(() => {
		const nextValue = value;

		instance?.updateOptions({ readOnly });

		if (instance && instance.getValue() !== nextValue) {
			instance.setValue(nextValue);
		}
	});
</script>

<div
	class={twMerge(
		'code-editor relative h-full min-h-0 w-full bg-surface text-on-surface',
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
						: 2}>{#each tokens as line, index (index)}{#each line as token (token.offset)}<span
								style:color={token.color}>{token.content}</span
							>{/each}{index < tokens.length - 1 ? '\n' : ''}{/each}{value.endsWith('\n')
						? '\n'
						: ''}</pre>
				{#if mounted}
					<textarea
						bind:this={fallback}
						bind:value
						readonly={readOnly}
						aria-label={`${language} code editor`}
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

	.code-editor :global(.monaco-editor .margin-view-overlays .current-line) {
		background-color: transparent !important;
	}

	.code-editor :global(.monaco-editor .selected-text),
	.code-editor :global(.monaco-editor.focused .view-overlays .current-line),
	.code-editor :global(.monaco-editor.focused .margin-view-overlays .current-line-margin-both) {
		background-color: var(--editor-selection) !important;
	}
</style>
