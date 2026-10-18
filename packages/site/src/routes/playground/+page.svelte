<script lang="ts">
	import { Button } from '#lib/ui/components/button';
	import Toolbar from './(components)/toolbar.svelte';
	import EditorWorkspace from './(components)/editor-workspace.svelte';
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
	class="flex h-dvh min-h-0 flex-col overflow-hidden border-b border-ink-outline bg-ink-surface text-on-ink-surface"
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
	<main aria-label="Playground editors" class="min-h-0 flex-1">
		<EditorWorkspace
			{playground}
			bind:rust={playground.source.rust}
			bind:typescript={playground.source.typescript}
		/>
	</main>
</div>
