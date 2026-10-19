<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { SvelteURL } from 'svelte/reactivity';
	import { X } from '@lucide/svelte';
	import { Button } from '#lib/ui/components/button';
	import Toolbar from './(components)/toolbar.svelte';
	import EditorWorkspace from './(components)/editor-workspace.svelte';
	import { Playground } from './(components)/playground.svelte';

	const playground = new Playground(() => page.url.searchParams.has('inspect'));
	let showRunNotice = $state(false);

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
		onrun={() => (showRunNotice = true)}
	/>
	{#if showRunNotice}
		<div
			role="status"
			class="flex shrink-0 items-center justify-between gap-4 border-y border-rust-outline bg-rust-surface px-6 py-2 text-ui text-on-ink-surface"
		>
			<p>Run is not connected yet. You can edit both files and explore the examples.</p>
			<Button
				variant="ghost"
				size="icon"
				aria-label="Dismiss message"
				onclick={() => (showRunNotice = false)}
			>
				<X aria-hidden="true" class="size-4" />
			</Button>
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
