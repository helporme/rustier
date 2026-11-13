<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { SvelteURL } from 'svelte/reactivity';
	import Toolbar from '../(components)/toolbar.svelte';
	import PlaygroundWorkspace from '../(components)/editors/playground-workspace.svelte';
	import { Playground } from '../(components)/playground.svelte';
	import { slugify } from '../(components)/snippets/share';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const playground = new Playground(() => data.selection);

	async function save(title: string, copy: boolean) {
		const url = new SvelteURL(page.url.href);
		url.pathname = resolve('/playground/[[slug]]', {});
		const href = await playground.save(url, title, copy);
		if (!href) return false;
		const savedUrl = new SvelteURL(href);
		await goto(
			resolve(`/playground/[[slug]]?${savedUrl.searchParams}${savedUrl.hash}`, {
				slug: slugify(title.trim())
			}),
			{ reset: false, state: page.state }
		);
		return true;
	}
</script>

<svelte:head>
	<title>{playground.title} — Rustier playground</title>
	<meta
		name="description"
		content="Explore inline Rust in TypeScript with the Rustier playground."
	/>
</svelte:head>

<div
	class="flex h-dvh min-h-0 flex-col overflow-hidden border-b border-ink-outline bg-rust-surface text-on-ink-surface"
>
	<Toolbar
		selected={playground.selected}
		title={playground.title}
		saved={playground.saved}
		pending={playground.compilation.pending}
		onrun={playground.run}
		onsave={save}
		saving={playground.saving}
		copied={playground.copied}
		saveError={playground.saveError}
	/>
	{#if data.shareError}
		<p role="alert" class="border-b border-ink-outline px-6 py-2 text-ui">{data.shareError}</p>
	{/if}
	<main aria-label="Playground editors" class="min-h-0 flex-1">
		<PlaygroundWorkspace {playground} />
	</main>
</div>
