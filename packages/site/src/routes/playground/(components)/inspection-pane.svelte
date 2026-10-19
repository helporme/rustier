<script lang="ts" module>
	import { Braces, Cpu, FileCode, FileType, Library, TriangleAlert } from '@lucide/svelte';
	import type { EditorTab } from './editor-pane.svelte';
	import type { Inspection } from '#lib/playground/inspector';

	const outputs = [
		{
			id: 'js_code',
			label: 'JS Bridge',
			language: 'typescript',
			filename: 'lib.rs.js',
			icon: FileCode
		},
		{
			id: 'rust_code',
			label: 'Rust Bridge',
			language: 'rust',
			filename: 'bridge.rs',
			icon: Braces
		},
		{ id: 'ts_code', label: 'TS', language: 'typescript', filename: 'lib.rs.d.ts', icon: FileType },
		{
			id: 'ambient_ts_code',
			label: 'TS Ambient',
			language: 'typescript',
			filename: 'ambient.d.ts',
			icon: Library
		},
		{
			id: 'runtime_js_code',
			label: 'JS Runtime',
			language: 'typescript',
			filename: '$rust/unstable!/wasm',
			icon: Cpu
		}
	] as const satisfies readonly (EditorTab & { id: keyof Inspection; filename: string })[];

	const diagnosticsTab = {
		id: 'diagnostics',
		label: 'Diagnostics',
		language: 'text',
		filename: 'diagnostics.txt',
		icon: TriangleAlert
	} as const satisfies EditorTab & { id: keyof Inspection; filename: string };
</script>

<script lang="ts">
	import { Button } from '#lib/ui/components/button';
	import EditorPane from './editor-pane.svelte';
	import type { Inspector } from '#lib/playground/inspector';

	let {
		inspector,
		selected = $bindable('js_code')
	}: {
		inspector: Inspector;
		selected: string;
	} = $props();

	const diagnostics = $derived(inspector.error || inspector.compilation?.diagnostics || '');
	const diagnosticCount = $derived(
		inspector.error ? 1 : (inspector.compilation?.diagnostic_count ?? 0)
	);
	const tabs = $derived(
		diagnostics
			? [...outputs, { ...diagnosticsTab, label: `Diagnostics [${diagnosticCount}]` }]
			: outputs
	);
	const output = $derived(tabs.find((output) => output.id === selected) ?? outputs[0]);
	const content = $derived(
		inspector.pending
			? '/* Generating... */'
			: output.id === 'diagnostics'
				? diagnostics
				: (inspector.compilation?.[output.id] ?? '')
	);
</script>

<div role="status" aria-atomic="true" class="sr-only">
	{diagnostics
		? `${diagnosticCount} ${diagnosticCount === 1 ? 'diagnostic' : 'diagnostics'} available in the Diagnostics tab.`
		: ''}
</div>

<EditorPane
	readonly
	{tabs}
	bind:selected={() => output.id, (value) => (selected = value)}
	value={content}
	filename={output.filename}
	language={output.language}
	label="Generated output"
>
	{#snippet status()}
		{#if !inspector.pending && output.id === 'diagnostics' && inspector.error}
			<div class="shrink-0 border-b border-ink-outline px-4 py-2">
				<Button variant="outline" onclick={inspector.restart}>Restart compiler</Button>
			</div>
		{/if}
	{/snippet}
</EditorPane>
