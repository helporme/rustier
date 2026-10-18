<script lang="ts">
	import { Button } from '#lib/ui/components/button';
	import Toolbar from './(components)/toolbar.svelte';
	import EditorPane from './(components)/editor-pane.svelte';
	import { Playground } from './(components)/playground.svelte';

	const playground = new Playground();
	let showRunNotice = $state(false);
</script>

<svelte:head>
	<title>Rustier playground — JavaScript, but rustier</title>
	<meta
		name="description"
		content="Explore Rust and TypeScript side by side in the Rustier playground."
	/>
</svelte:head>

<div
	class="flex min-h-dvh flex-col border-b border-ink-outline bg-ink-surface text-on-ink-surface md:h-dvh md:overflow-hidden"
>
	<Toolbar bind:selected={playground.selected} onrun={() => (showRunNotice = true)} />
	{#if showRunNotice}
		<div
			role="status"
			class="flex items-center justify-between gap-4 bg-primary-surface px-6 py-2 text-ui text-on-primary-surface"
		>
			<p>Run is not connected yet. You can edit both files and explore the examples.</p>
			<Button
				variant="ghost"
				size="icon"
				aria-label="Dismiss message"
				onclick={() => (showRunNotice = false)}>×</Button
			>
		</div>
	{/if}
	<main
		aria-label="Playground editors"
		class="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-2 [&>section+section]:border-t [&>section+section]:border-ink-outline md:[&>section+section]:border-t-0 md:[&>section+section]:border-l"
	>
		{#key playground.selected}
			<EditorPane language="rust" filename="lib.rs" bind:value={playground.source.rust} />
			<EditorPane
				language="typescript"
				filename="index.ts"
				bind:value={playground.source.typescript}
			/>
		{/key}
	</main>
</div>
