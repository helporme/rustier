<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { Snippet } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import { Button } from '#lib/ui/components/button';
	import { Toggle } from '#lib/ui/components/toggle';
	import { ThemeToggle } from '#lib/ui/components/theme-toggle';
	import { cn } from '#lib/ui/utils';

	let {
		examples,
		selected,
		mode,
		onModeChange,
		building,
		onBuild,
		output,
		showOutput,
		onCloseOutput,
		error,
		onRestart
	}: {
		examples: { name: string; slug: string; group: string }[];
		selected: string;
		mode: 'run' | 'analysis';
		onModeChange: (mode: 'run' | 'analysis') => void;
		building: boolean;
		onBuild: () => void;
		output: Snippet;
		showOutput: boolean;
		onCloseOutput: () => void;
		error?: string;
		onRestart: () => void;
	} = $props();

	const id = $props.id();
	let buildButton = $state<HTMLButtonElement | HTMLAnchorElement | null>(null);
	let groups = $derived([...new Set(examples.map((example) => example.group))]);

	function href(example: string) {
		const search = new SvelteURLSearchParams(page.url.search);

		const query = search.toString();
		const route = query
			? (`/playground/[[example]]?${query}${page.url.hash}` as const)
			: page.url.hash
				? (`/playground/[[example]]${page.url.hash}` as `/playground/[[example]]#${string}`)
				: '/playground/[[example]]';

		return resolve(route, { example });
	}
</script>

<aside
	aria-label="Playground navigation"
	class="flex min-h-0 min-w-0 flex-col overflow-y-auto bg-[#000] px-[1ch] text-on-surface scheme-dark"
>
	<div aria-hidden="true" class="overflow-hidden whitespace-nowrap text-on-surface-alt">
		{'-'.repeat(80)}
	</div>
	<h1 class="pl-[1ch] font-normal">rustier playground (beta)</h1>
	<div aria-hidden="true" class="overflow-hidden whitespace-nowrap text-on-surface-alt">
		{'-'.repeat(80)}
	</div>
	<nav aria-label="Playground controls" class="flex items-start gap-[1ch]">
		<Button
			bind:ref={buildButton}
			variant="foreground"
			color="green"
			disabled={building}
			aria-busy={building}
			onclick={onBuild}
			class="gap-[1ch]"
		>
			<span aria-hidden="true">[>]</span>
			Run
		</Button>
		<Toggle
			color="magenta"
			pressed={mode === 'analysis'}
			onPressedChange={(pressed) => onModeChange(pressed ? 'analysis' : 'run')}
			class="gap-[1ch]"
		>
			<span aria-hidden="true">[?]</span>
			Inspect
		</Toggle>
		<ThemeToggle />
	</nav>
	<div aria-hidden="true" class="overflow-hidden whitespace-nowrap text-on-surface-alt">
		{'-'.repeat(80)}
	</div>
	<section
		aria-labelledby={`${id}-output`}
		class={showOutput ? 'flex min-h-0 flex-1 flex-col' : 'hidden'}
	>
		<div class="flex items-center justify-between">
			<h2 id={`${id}-output`} class="font-normal text-on-surface-alt">Output</h2>
			<Button onclick={onCloseOutput} class="gap-[1ch] text-on-surface-alt">
				<span aria-hidden="true">[x]</span>
				Close
			</Button>
		</div>
		<div aria-hidden="true" class="overflow-hidden whitespace-nowrap text-on-surface-alt">
			{'-'.repeat(80)}
		</div>
		<div
			class="mx-[-1ch] min-h-0 flex-1"
			onkeydowncapture={(event) => {
				if (event.key !== 'Escape') return;

				event.preventDefault();
				event.stopPropagation();
				onCloseOutput();
				buildButton?.focus({ preventScroll: true });
			}}
		>
			{@render output()}
		</div>
	</section>
	{#if !showOutput}
		<nav aria-labelledby={`${id}-examples`} data-sveltekit-preload-data="hover">
			<h2 id={`${id}-examples`} class="font-normal">Examples</h2>
			{#each groups as group (group)}
				<section aria-labelledby={`${id}-${group}`}>
					<h3 id={`${id}-${group}`} class="pl-[2ch] font-normal text-on-surface-alt/65">
						{group}
					</h3>
					<ul role="list">
						{#each examples.filter((example) => example.group === group) as example (example.slug)}
							<li>
								<Button
									prefix
									href={href(example.slug)}
									aria-current={selected === example.slug ? 'page' : undefined}
									class={cn(
										'flex w-full items-start pr-0 pl-[4ch] whitespace-normal text-magenta hover:bg-transparent focus-visible:bg-transparent focus-visible:outline-magenta',
										selected === example.slug && 'font-bold underline'
									)}
								>
									<span class="min-w-0">{example.name}</span>
								</Button>
							</li>
						{/each}
					</ul>
				</section>
			{/each}
		</nav>
	{/if}
	{#if error}
		<div role="alert" class="mt-auto pt-4">
			<p>Compiler unavailable</p>
			<Button
				onclick={onRestart}
				class="text-magenta hover:bg-transparent focus-visible:bg-transparent focus-visible:outline-magenta"
				>Retry compiler</Button
			>
		</div>
	{/if}
</aside>
