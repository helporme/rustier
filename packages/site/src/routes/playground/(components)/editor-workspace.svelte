<script lang="ts">
	import * as Resizable from '#lib/ui/components/resizable';
	import EditorPane from './editor-pane.svelte';
	import InspectionPane from './inspection-pane.svelte';
	import SourcePane from './source-pane.svelte';
	import type { Playground } from './playground.svelte';

	let {
		playground,
		rust = $bindable(''),
		typescript = $bindable('')
	}: {
		playground: Playground;
		rust: string;
		typescript: string;
	} = $props();

	const snap = new Resizable.CenterSnap();
</script>

<div class="h-full min-h-0 min-w-0 md:hidden">
	{#key playground.selected}
		<SourcePane
			inspector={playground.inspector}
			inspect={playground.inspect}
			bind:selected={playground.mobileTab}
			bind:rust
			bind:typescript
		/>
	{/key}
</div>

<Resizable.PaneGroup
	bind:ref={snap.group}
	direction="horizontal"
	keyboardResizeBy={5}
	onLayoutChange={snap.onLayoutChange}
	class="hidden! md:flex!"
>
	<Resizable.Pane bind:this={snap.pane} defaultSize={50} minSize={20} class="min-h-0 min-w-0">
		{#key playground.selected}
			{#if playground.inspect}
				<SourcePane
					inspector={playground.inspector}
					bind:selected={playground.sourceTab}
					bind:rust
					bind:typescript
				/>
			{:else}
				<EditorPane language="rust" filename="lib.rs" bind:value={rust} />
			{/if}
		{/key}
	</Resizable.Pane>
	<Resizable.Handle
		aria-label="Resize editors"
		aria-orientation="vertical"
		class="focus-visible:ring-rust data-[active=pointer]:bg-rust data-[active=pointer]:ring-rust"
	/>
	<Resizable.Pane defaultSize={50} minSize={20} class="min-h-0 min-w-0">
		{#key playground.selected}
			{#if playground.inspect}
				<InspectionPane inspector={playground.inspector} bind:selected={playground.outputTab} />
			{:else}
				<EditorPane language="typescript" filename="index.ts" bind:value={typescript} />
			{/if}
		{/key}
	</Resizable.Pane>
</Resizable.PaneGroup>
