<script lang="ts">
	import { CodeEditor, type CodeLanguage } from '#lib/ui/components/code-editor';
	import rustIcon from '#lib/assets/rust.svg';
	import typescriptIcon from '#lib/assets/typescript.svg';

	let {
		language,
		filename,
		value = $bindable('')
	}: {
		language: CodeLanguage;
		filename: string;
		value: string;
	} = $props();

	const name = $derived(language === 'rust' ? 'Rust' : 'TypeScript');
	const id = $props.id();
</script>

<section
	aria-label={`${name} workspace`}
	style:view-transition-name={language === 'rust' ? 'rust-editor' : 'typescript-editor'}
	class="flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-ink-surface-bright"
>
	<div
		class="flex h-9 shrink-0 items-stretch bg-ink-surface after:flex-1 after:border-b after:border-ink-outline"
	>
		<h2
			{id}
			class="flex min-w-30 items-center gap-2 border-r border-ink-outline bg-ink-surface-bright px-4 font-code text-ui"
		>
			{#if language === 'rust'}
				<span
					aria-hidden="true"
					class="size-4 shrink-0 bg-current"
					style:mask={`url(${rustIcon}) center / contain no-repeat`}
				></span>
			{:else}
				<img src={typescriptIcon} alt="" class="size-4 shrink-0" />
			{/if}
			{filename}
		</h2>
	</div>
	<div class="min-h-0 flex-1" role="region" aria-labelledby={id}>
		<CodeEditor bind:value {language} label={`${name} editor`} />
	</div>
</section>
