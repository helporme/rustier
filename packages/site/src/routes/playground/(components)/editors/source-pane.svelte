<script lang="ts" module>
	const sources = [
		{ id: 'lib.rs', label: 'lib.rs', filename: 'lib.rs', language: 'rust' },
		{ id: 'index.ts', label: 'index.ts', filename: 'index.ts', language: 'typescript' }
	] as const;
</script>

<script lang="ts">
	import { Button } from '#lib/ui/components/button';
	import type { Inspector } from '#lib/playground/inspector';
	import EditorPane from './editor-pane.svelte';
	import { inspectionView } from './inspection-view';

	let {
		inspector,
		inspect = false,
		selected = $bindable('lib.rs'),
		rust = $bindable(''),
		typescript = $bindable('')
	}: {
		inspector: Inspector;
		inspect?: boolean;
		selected?: string;
		rust: string;
		typescript: string;
	} = $props();

	const view = $derived(inspectionView(inspector, selected));
	const tabs = $derived(inspect ? [...sources, ...view.tabs] : sources);
	const file = $derived(tabs.find((tab) => tab.id === selected) ?? sources[0]);
	const readonly = $derived(file.id !== 'lib.rs' && file.id !== 'index.ts');
</script>

{#if inspect}
	<div role="status" aria-atomic="true" class="sr-only">{view.status}</div>
{/if}

<EditorPane
	{tabs}
	bind:selected={() => file.id, (value) => (selected = value)}
	bind:value={
		() => (file.id === 'lib.rs' ? rust : file.id === 'index.ts' ? typescript : view.content),
		(value) => {
			if (file.id === 'lib.rs') rust = value;
			else if (file.id === 'index.ts') typescript = value;
		}
	}
	{readonly}
	filename={file.filename}
	language={file.language}
	label={inspect ? 'Workspace files' : 'Source files'}
>
	{#snippet status()}
		{#if inspect && readonly && view.restart}
			<div class="shrink-0 border-b border-ink-outline px-4 py-2">
				<Button variant="outline" onclick={inspector.restart}>Restart compiler</Button>
			</div>
		{/if}
	{/snippet}
</EditorPane>
