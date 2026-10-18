<script lang="ts" module>
	import { cn } from '#lib/ui/utils';
	import { highlight, type CodeLanguage } from './highlight';
	import { CodeEditorController } from './editor.svelte';

	export type CodeEditorProps = {
		value?: string;
		language: CodeLanguage;
		label: string;
		readonly?: boolean;
		disabled?: boolean;
		placeholder?: string;
		class?: string;
		onchange?: (value: string) => void;
	};
</script>

<script lang="ts">
	let {
		value = $bindable(''),
		language,
		label,
		readonly = false,
		disabled = false,
		placeholder = '',
		class: className,
		onchange
	}: CodeEditorProps = $props();

	const editor = new CodeEditorController({
		get value() {
			return value;
		},
		get language() {
			return language;
		},
		get label() {
			return label;
		},
		get readonly() {
			return readonly;
		},
		get disabled() {
			return disabled;
		},
		get placeholder() {
			return placeholder;
		},
		onchange(next) {
			value = next;
			onchange?.(next);
		}
	});
	const tokens = $derived(editor.ready ? [] : highlight(value, language));
</script>

<div
	data-slot="code-editor"
	data-ready={editor.ready}
	data-disabled={disabled || undefined}
	class={cn(
		'relative h-full min-h-0 min-w-0 bg-ink-surface-bright font-code text-code text-on-ink-surface data-disabled:pointer-events-none data-disabled:opacity-50',
		'[&_.cm-editor]:h-full [&_.cm-editor]:bg-transparent! [&_.cm-editor]:outline-none!',
		'[&_.cm-content]:py-4.5! [&_.cm-content]:caret-on-ink-surface! [&_.cm-cursor]:border-l-on-ink-surface! [&_.cm-line]:px-4! [&_.cm-scroller]:overflow-auto [&_.cm-scroller]:font-code! [&_.cm-scroller]:leading-normal!',
		'[&_.cm-gutterElement]:pr-1! [&_.cm-gutterElement]:text-line-number! [&_.cm-gutters]:w-8! [&_.cm-gutters]:shrink-0 [&_.cm-gutters]:border-0! [&_.cm-gutters]:bg-ink-surface-bright! [&_.cm-gutters]:text-on-ink-surface-variant! [&_.cm-lineNumbers]:w-full',
		'[&_.cm-activeLine]:bg-transparent! [&_.cm-content::selection]:bg-editor-selection! [&_.cm-focused_.cm-activeLine]:bg-editor-active-line! [&_.cm-placeholder]:text-on-ink-surface-variant! [&_.cm-selectionBackground]:bg-editor-selection!',
		'[&_.cm-button]:border-primary-outline! [&_.cm-button]:bg-primary! [&_.cm-button]:bg-none! [&_.cm-button]:text-on-primary! [&_.cm-panels]:border-ink-outline! [&_.cm-panels]:bg-ink-surface! [&_.cm-panels]:font-body! [&_.cm-panels]:text-ui! [&_.cm-panels]:text-on-ink-surface! [&_.cm-textfield]:rounded [&_.cm-textfield]:border-ink-outline! [&_.cm-textfield]:bg-ink-surface-bright! [&_.cm-textfield]:text-on-ink-surface!',
		'[&_.cm-tooltip]:border-ink-outline! [&_.cm-tooltip]:bg-ink-surface-bright! [&_.cm-tooltip]:text-on-ink-surface! [&_.cm-tooltip-autocomplete_[aria-selected=true]]:bg-primary! [&_.cm-tooltip-autocomplete_[aria-selected=true]]:text-on-primary!',
		className
	)}
>
	<div class="h-full" {@attach editor.mount}></div>
	{#if !editor.ready}
		<div class="absolute inset-0">
			<div
				data-line-numbers
				aria-hidden="true"
				class="pointer-events-none absolute inset-y-0 left-0 z-10 w-8 overflow-hidden bg-ink-surface-bright py-4.5 pr-1 text-right text-line-number text-on-ink-surface-variant"
			>
				{#each value.split('\n').keys() as index (index)}
					<div>{index + 1}</div>
				{/each}
			</div>
			<pre
				aria-hidden="true"
				style:tab-size={language === 'rust' ? 4 : 2}
				class="pointer-events-none absolute inset-0 overflow-hidden py-4.5 pr-4 pl-12 whitespace-pre">{#each tokens as token (token.from)}<span
						class={token.className}>{token.text}</span
					>{/each}{value.endsWith('\n') ? '\n' : ''}</pre>
			<textarea
				bind:value
				aria-label={label}
				{readonly}
				{disabled}
				{placeholder}
				spellcheck="false"
				wrap="off"
				oninput={() => onchange?.(value)}
				onscroll={(event) => {
					const preview = event.currentTarget.previousElementSibling;
					if (!preview) return;
					preview.scrollTop = event.currentTarget.scrollTop;
					preview.scrollLeft = event.currentTarget.scrollLeft;
					const numbers = preview.previousElementSibling;
					if (numbers) numbers.scrollTop = event.currentTarget.scrollTop;
				}}
				style:tab-size={language === 'rust' ? 4 : 2}
				class="absolute inset-0 h-full w-full resize-none bg-transparent py-4.5 pr-4 pl-12 text-transparent caret-on-ink-surface outline-none placeholder:text-on-ink-surface-variant"
			></textarea>
		</div>
	{/if}
	{#if editor.failed}
		<p
			role="status"
			class="absolute right-2 bottom-2 rounded bg-ink-surface px-2 py-1 font-body text-ui text-on-ink-surface-variant"
		>
			Editor unavailable. You can still edit plain text.
		</p>
	{/if}
</div>
