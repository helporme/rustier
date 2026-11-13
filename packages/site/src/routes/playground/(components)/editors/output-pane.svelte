<script lang="ts" module>
	import type { CodeLanguage } from '#lib/ui/components/code-editor';
	import type { Analysis } from '#lib/playground/analysis';
	import { Terminal, TriangleAlert } from '@lucide/svelte';
	import javascriptIcon from '#lib/assets/javascript.svg';
	import rustIcon from '#lib/assets/rust.svg';

	const analysisTabs = [
		{ id: 'transformedJs', label: 'Transformed JS', language: 'typescript', icon: javascriptIcon },
		{ id: 'rustGlue', label: 'Rust Glue', language: 'rust', icon: rustIcon },
		{ id: 'jsGlue', label: 'JS Glue', language: 'typescript', icon: javascriptIcon }
	] as const satisfies readonly {
		id: keyof Analysis;
		label: string;
		language: CodeLanguage;
		icon: string;
	}[];
</script>

<script lang="ts">
	import * as Tabs from '#lib/ui/components/tabs';
	import { CodeEditor } from '#lib/ui/components/code-editor';
	import ExecutionOutput from '../terminal/execution-output.svelte';
	import type { Compilation } from '#lib/playground/compilation';
	import type { Analyzer } from '#lib/playground/analysis';
	import { Button } from '#lib/ui/components/button';

	let {
		compilation,
		analyzer,
		source,
		selected = $bindable('results')
	}: {
		compilation: Compilation;
		analyzer: Analyzer;
		source: string;
		selected: string;
	} = $props();
</script>

{#snippet status(message: string)}
	<p role="status" class="px-6 pt-3 pb-4 font-mono text-code text-on-ink-surface-variant italic">
		{message}
	</p>
{/snippet}

<section
	aria-label="Playground output"
	class="flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-ink-surface-bright [view-transition-name:playground-output]"
>
	<Tabs.Root bind:value={selected} class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
		<Tabs.List aria-label="Output tabs">
			<Tabs.Trigger value="results">
				<Terminal aria-hidden="true" class="size-4 shrink-0" />
				Results
			</Tabs.Trigger>
			{#each analysisTabs as tab (tab.id)}
				<Tabs.Trigger value={tab.id}>
					<span
						aria-hidden="true"
						class="size-4 shrink-0 bg-current"
						style:mask={`url(${JSON.stringify(tab.icon)}) center / contain no-repeat`}
					></span>
					{tab.label}
				</Tabs.Trigger>
			{/each}
			<Tabs.Trigger value="diagnostics">
				<TriangleAlert aria-hidden="true" class="size-4 shrink-0" />
				Diagnostics
			</Tabs.Trigger>
		</Tabs.List>
		{#if selected === 'results' && compilation.source !== undefined && compilation.source !== source}
			<p role="status" class="shrink-0 border-b border-ink-outline px-4 py-2 text-ui">
				Source changed. Run again to update these results.
			</p>
		{/if}
		<Tabs.Content value="results">
			<ExecutionOutput {compilation} />
		</Tabs.Content>
		{#if selected !== 'results' && analyzer.error}
			<div role="alert" class="border-b border-ink-outline px-4 py-2 text-ui">
				<p>{analyzer.error}</p>
				<Button variant="outline" onclick={analyzer.restart}>Restart analysis</Button>
			</div>
		{/if}
		<Tabs.Content value="diagnostics">
			{#if selected === 'diagnostics'}
				{#if analyzer.pending}
					{@render status('Analyzing in browser…')}
				{:else if analyzer.output?.diagnostics}
					<CodeEditor
						readonly
						value={analyzer.output.diagnostics}
						language="diagnostics"
						label="Analysis diagnostics"
					/>
				{:else if analyzer.output}
					{@render status('No analysis diagnostics.')}
				{/if}
			{/if}
		</Tabs.Content>
		{#each analysisTabs as tab (tab.id)}
			<Tabs.Content value={tab.id}>
				{#if selected === tab.id}
					{#if analyzer.output}
						<CodeEditor
							readonly
							value={analyzer.output[tab.id]}
							language={tab.language}
							label={`${tab.label} output`}
						/>
					{:else}
						{@render status(analyzer.pending ? 'Analyzing in browser…' : 'Analysis unavailable.')}
					{/if}
				{/if}
			</Tabs.Content>
		{/each}
	</Tabs.Root>
</section>
