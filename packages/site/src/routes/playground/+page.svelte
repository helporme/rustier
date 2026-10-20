<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { SvelteURL } from 'svelte/reactivity';
	import Toolbar from './(components)/toolbar.svelte';
	import EditorWorkspace from './(components)/editor-workspace.svelte';
	import TerminalWorkspace from './(components)/terminal-workspace.svelte';
	import { Playground } from './(components)/playground.svelte';

	const playground = new Playground(() => page.url.searchParams.has('inspect'));

	function setInspect(inspect: boolean) {
		const url = new SvelteURL(page.url.href);

		url.searchParams.delete('inspect');
		if (inspect) url.search += `${url.search ? '&' : '?'}inspect`;

		const route = url.search
			? (`/playground${url.search}${url.hash}` as `/playground?${string}`)
			: url.hash
				? (`/playground${url.hash}` as `/playground#${string}`)
				: '/playground';

		void goto(resolve(route), {
			reset: false,
			state: page.state
		});
	}
</script>

<svelte:head>
	<title>Rustier playground</title>
	<meta
		name="description"
		content="Explore Rust and TypeScript side by side in the Rustier playground."
	/>
</svelte:head>

<div
	class="flex h-dvh min-h-0 flex-col overflow-hidden border-b border-ink-outline bg-rust-surface text-on-ink-surface"
>
	<Toolbar
		bind:selected={playground.selected}
		bind:inspect={() => playground.inspect, setInspect}
		pending={playground.compilation.pending}
		onrun={playground.run}
	/>
	<main aria-label="Playground editors" class="min-h-0 flex-1">
		<TerminalWorkspace terminal={playground.terminal} compilation={playground.compilation}>
			<EditorWorkspace
				{playground}
				bind:rust={playground.source.rust}
				bind:typescript={playground.source.typescript}
			/>
		</TerminalWorkspace>
	</main>
</div>
