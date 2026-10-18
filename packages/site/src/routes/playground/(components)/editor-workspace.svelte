<script lang="ts">
	import * as Resizable from '#lib/ui/components/resizable';
	import EditorPane from './editor-pane.svelte';
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

<Resizable.PaneGroup
	bind:ref={snap.group}
	direction={playground.direction}
	keyboardResizeBy={5}
	onLayoutChange={snap.onLayoutChange}
	class="max-md:flex-col!"
>
	<Resizable.Pane bind:this={snap.pane} defaultSize={50} minSize={20} class="min-h-0 min-w-0">
		{#key playground.selected}
			<EditorPane language="rust" filename="lib.rs" bind:value={rust} />
		{/key}
	</Resizable.Pane>
	<Resizable.Handle
		aria-label="Resize editors"
		aria-orientation={playground.direction === 'horizontal' ? 'vertical' : 'horizontal'}
		class="focus-visible:ring-rust data-[active=pointer]:bg-rust data-[active=pointer]:ring-rust max-md:h-px! max-md:w-full!"
	/>
	<Resizable.Pane defaultSize={50} minSize={20} class="min-h-0 min-w-0">
		{#key playground.selected}
			<EditorPane language="typescript" filename="index.ts" bind:value={typescript} />
		{/key}
	</Resizable.Pane>
</Resizable.PaneGroup>
