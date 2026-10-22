<script lang="ts">
	import { Check, ChevronDown, X } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import { Button } from '#lib/ui/components/button';
	import * as Popover from '#lib/ui/components/popover';
	import { exampleGroups } from './examples';
	import { sharedId, slugify } from '../snippets/share';
	import type { SavedSnippets } from '../snippets/saved-snippets.svelte';

	let { selected, title, saved }: { selected: string; title: string; saved: SavedSnippets } =
		$props();
	let open = $state(false);
	const id = $props.id();

	function route(code?: string): '/playground/[[slug]]' | `/playground/[[slug]]?${string}` {
		const params = new SvelteURLSearchParams();
		if (code) params.set('code', code);
		if (page.url.searchParams.has('inspect')) params.set('inspect', '');
		const query = params.toString();
		return query ? `/playground/[[slug]]?${query}` : '/playground/[[slug]]';
	}
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button
				{...props}
				variant="outline"
				class="w-52 max-w-full min-w-0 shrink justify-between px-2 sm:px-3.5"
				aria-label={`Example: ${title}`}
			>
				<span class="truncate">{title}</span>
				<ChevronDown aria-hidden="true" class="size-4 shrink-0" />
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content aria-label="Examples" class="p-1">
		<nav aria-label="Playground examples">
			{#if saved.entries.length}
				<section aria-labelledby={`${id}-mine`}>
					<h2 id={`${id}-mine`} class="px-3 pt-2 pb-1 font-bold text-on-ink-surface-variant">
						My snippets
					</h2>
					{#each saved.entries as snippet (snippet.code)}
						<div class="flex items-center">
							<a
								href={resolve(route(snippet.code), {
									slug: slugify(snippet.source.title)
								})}
								onclick={() => (open = false)}
								aria-current={selected === sharedId(snippet.code) ? 'page' : undefined}
								class="relative flex min-h-9 min-w-0 flex-1 items-center py-1.5 pr-3 pl-8 outline-none hover:bg-java hover:text-ink focus-visible:bg-java focus-visible:text-ink aria-[current=page]:font-bold"
							>
								{#if selected === sharedId(snippet.code)}
									<Check aria-hidden="true" class="absolute left-2 size-3" />
								{/if}
								<span class="truncate">{snippet.source.title}</span>
							</a>
							<Button
								variant="ghost"
								size="icon"
								aria-label={`Forget ${snippet.source.title}`}
								title="Forget snippet"
								onclick={() => saved.forget(snippet.code)}
							>
								<X aria-hidden="true" class="size-4" />
							</Button>
						</div>
					{/each}
				</section>
			{/if}
			{#each exampleGroups as group, index (group.topic)}
				<section
					aria-labelledby={`${id}-group-${index}`}
					class="not-first:mt-2 not-first:border-t not-first:border-ink-outline"
				>
					<h2
						id={`${id}-group-${index}`}
						class="px-3 pt-2 pb-1 font-bold text-on-ink-surface-variant"
					>
						{group.topic}
					</h2>
					{#each group.examples as example (example.id)}
						<a
							href={resolve(route(), {
								slug: example.id
							})}
							onclick={() => (open = false)}
							aria-current={selected === example.id ? 'page' : undefined}
							class="relative flex min-h-9 items-center py-1.5 pr-3 pl-8 outline-none hover:bg-java hover:text-ink focus-visible:bg-java focus-visible:text-ink aria-[current=page]:font-bold"
						>
							{#if selected === example.id}
								<Check aria-hidden="true" class="absolute left-2 size-3" />
							{/if}
							{example.name}
						</a>
					{/each}
				</section>
			{/each}
		</nav>
		{#if saved.error}
			<p role="alert" class="px-3 py-2">{saved.error}</p>
		{/if}
	</Popover.Content>
</Popover.Root>
