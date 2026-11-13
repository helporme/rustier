<script lang="ts">
	import { MediaQuery } from 'svelte/reactivity';
	import * as Resizable from '#lib/ui/components/resizable';
	import TerminalWorkspace from '../terminal/terminal-workspace.svelte';
	import EditorWorkspace from './editor-workspace.svelte';
	import OutputPane from './output-pane.svelte';
	import type { Playground } from '../playground.svelte';

	let { playground }: { playground: Playground } = $props();
	const desktop = new MediaQuery('(min-width: 48rem)', true);
	const snap = new Resizable.CenterSnap();
</script>

{#snippet source()}
	<EditorWorkspace {playground} bind:typescript={playground.source.typescript} />
{/snippet}

{#snippet output()}
	<OutputPane
		compilation={playground.compilation}
		analyzer={playground.analyzer}
		source={playground.source.typescript}
		bind:selected={playground.outputTab}
	/>
{/snippet}

{#if desktop.current}
	<Resizable.PaneGroup
		direction="horizontal"
		bind:ref={snap.group}
		onLayoutChange={snap.onLayoutChange}
		keyboardResizeBy={5}
	>
		<Resizable.Pane bind:this={snap.pane} defaultSize={50} minSize={20} class="min-h-0 min-w-0">
			{@render source()}
		</Resizable.Pane>
		<Resizable.Handle aria-label="Resize output pane" aria-orientation="vertical" />
		<Resizable.Pane defaultSize={50} minSize={20} class="min-h-0 min-w-0">
			{@render output()}
		</Resizable.Pane>
	</Resizable.PaneGroup>
{:else}
	<TerminalWorkspace terminal={playground.terminal} {output}>
		{@render source()}
	</TerminalWorkspace>
{/if}
