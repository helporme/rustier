<script lang="ts">
	import EditorPane from './editor-pane.svelte';
	import type { Playground } from '../playground.svelte';

	let {
		playground,
		typescript = $bindable('')
	}: {
		playground: Playground;
		typescript: string;
	} = $props();
</script>

{#key playground.selected}
	<EditorPane language="typescript" filename="index.ts" bind:value={typescript} />
{/key}
