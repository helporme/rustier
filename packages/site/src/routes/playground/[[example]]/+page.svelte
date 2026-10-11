<script lang="ts">
	import { CodeEditor } from '#lib/ui/components/code-editor';
	import { examples } from '#lib/playground/examples';
	import Navigation from '../(components)/navigation.svelte';
	import Panels from '../(components)/panels.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let snippets = $state(examples.map((example) => ({ ...example })));
</script>

<div class="flex h-dvh min-h-0 flex-col bg-[light-dark(#fff,#000)] text-on-surface">
	<Navigation examples={snippets} selected={snippets[data.selected].slug} />
	<main class="@container min-h-0 min-w-0 flex-1 overflow-hidden" aria-label="playground">
		<Panels>
			{#snippet rust()}
				<CodeEditor bind:value={snippets[data.selected].code} language="rust" filename="lib.rs" />
			{/snippet}
			{#snippet typescript()}
				<CodeEditor
					bind:value={snippets[data.selected].typescript}
					language="typescript"
					filename="index.ts"
					class="bg-inherit"
				/>
			{/snippet}
			{#snippet results()}
				<CodeEditor
					value="# Results not available"
					language="shellscript"
					filename="output.sh"
					readOnly
					colorScheme="rustier-dark-dimmest"
					class="dark bg-surface-dim"
				/>
			{/snippet}
		</Panels>
	</main>
</div>
