<script lang="ts">
	import { resolve } from '$app/paths';
	import { Button } from '#lib/ui/components/button';
	import { ThemeToggle } from '#lib/ui/components/theme-toggle';
	import ButtonExamples from './(components)/button-examples.svelte';
	import SelectExamples from './(components)/select-examples.svelte';
	import EditorExamples from './(components)/editor-examples.svelte';
	import ThemeExamples from './(components)/theme-examples.svelte';
	import ResizableExamples from './(components)/resizable-examples.svelte';
	import IconExamples from './(components)/icon-examples.svelte';
	import TabsExamples from './(components)/tabs-examples.svelte';
	import ToggleExamples from './(components)/toggle-examples.svelte';
</script>

<svelte:head>
	<title>Rustier UI playground</title>
</svelte:head>

<main class="mx-auto max-w-7xl space-y-12 bg-ink-surface-bright p-6 text-on-ink-surface sm:p-10">
	<header class="flex flex-wrap items-center justify-between gap-4">
		<h1 class="font-brand text-3xl">UI playground</h1>
		<div class="flex flex-wrap items-center gap-3">
			<ThemeToggle />
			<Button href={resolve('/playground')} variant="outline">Back to playground</Button>
		</div>
	</header>
	<ThemeExamples />
	<IconExamples />
	<ButtonExamples />
	<ToggleExamples />
	<SelectExamples />
	<TabsExamples />
	<EditorExamples />
	<ResizableExamples />
</main>
