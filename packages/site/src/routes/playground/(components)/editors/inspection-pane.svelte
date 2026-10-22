<script lang="ts">
	import { Button } from '#lib/ui/components/button';
	import EditorPane from './editor-pane.svelte';
	import type { Inspector } from '#lib/playground/inspector';
	import { inspectionView } from './inspection-view';

	let {
		inspector,
		selected = $bindable('js_code')
	}: {
		inspector: Inspector;
		selected: string;
	} = $props();

	const view = $derived(inspectionView(inspector, selected));
</script>

<div role="status" aria-atomic="true" class="sr-only">{view.status}</div>

<EditorPane
	readonly
	tabs={view.tabs}
	bind:selected={() => view.output.id, (value) => (selected = value)}
	value={view.content}
	filename={view.output.filename}
	language={view.output.language}
	label="Generated output"
>
	{#snippet status()}
		{#if view.restart}
			<div class="shrink-0 border-b border-ink-outline px-4 py-2">
				<Button variant="outline" onclick={inspector.restart}>Restart compiler</Button>
			</div>
		{/if}
	{/snippet}
</EditorPane>
