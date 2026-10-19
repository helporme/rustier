<script lang="ts" module>
	import type { CodeLanguage } from '#lib/ui/components/code-editor';
	import type { Component } from 'svelte';
	import type { LucideProps } from '@lucide/svelte';

	export type EditorTab = {
		id: string;
		label: string;
		language: CodeLanguage;
		icon?: Component<LucideProps>;
	};
</script>

<script lang="ts">
	import { CodeEditor } from '#lib/ui/components/code-editor';
	import * as Tabs from '#lib/ui/components/tabs';
	import rustIcon from '#lib/assets/rust.svg';
	import typescriptIcon from '#lib/assets/typescript.svg';
	import type { Snippet } from 'svelte';

	let {
		language,
		filename,
		value = $bindable(''),
		selected = $bindable(''),
		tabs = [{ id: filename, label: filename, language }],
		readonly = false,
		label,
		status
	}: {
		language: CodeLanguage;
		filename: string;
		value: string;
		selected?: string;
		tabs?: readonly EditorTab[];
		readonly?: boolean;
		label?: string;
		status?: Snippet;
	} = $props();

	const name = $derived(language === 'rust' ? 'Rust' : language === 'text' ? 'Text' : 'TypeScript');
</script>

<section
	aria-label={label ?? `${name} workspace`}
	style:view-transition-name={readonly
		? undefined
		: language === 'rust'
			? 'rust-editor'
			: 'typescript-editor'}
	class="flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-ink-surface-bright"
>
	<Tabs.Root
		value={selected || filename}
		onValueChange={(next) => (selected = next)}
		class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
	>
		<Tabs.List aria-label={label ?? `${name} files`}>
			{#each tabs as tab, index (tab.id)}
				<Tabs.Trigger value={tab.id} enterDelay={30 * index}>
					{#if tab.icon}
						<tab.icon aria-hidden="true" class="size-4 shrink-0" />
					{:else if !readonly}
						{#if tab.language === 'rust'}
							<span
								aria-hidden="true"
								class="size-4 shrink-0 bg-current"
								style:mask={`url(${rustIcon}) center / contain no-repeat`}
							></span>
						{:else}
							<img src={typescriptIcon} alt="" class="size-4 shrink-0" />
						{/if}
					{/if}
					{tab.label}
				</Tabs.Trigger>
			{/each}
		</Tabs.List>
		{@render status?.()}
		{#each tabs as tab (tab.id)}
			<Tabs.Content value={tab.id}>
				{#if tab.id === (selected || filename)}
					<CodeEditor
						bind:value
						{language}
						{readonly}
						label={label ? `${filename} editor` : `${name} editor`}
					/>
				{/if}
			</Tabs.Content>
		{/each}
	</Tabs.Root>
</section>
