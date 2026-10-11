<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { Button } from '#lib/ui/components/button';
	import * as DropdownMenu from '#lib/ui/components/dropdown-menu';

	let {
		examples,
		selected
	}: {
		examples: { name: string; slug: string; group: string }[];
		selected: string;
	} = $props();

	let current = $derived(examples.find((example) => example.slug === selected)!);
	let groups = $derived([...new Set(examples.map((example) => example.group))]);
</script>

<header class="flex shrink-0 items-center gap-2 px-2 py-1 text-on-surface">
	<h1 class="flex shrink-0 items-center gap-2 font-normal text-on-surface">
		<span aria-hidden="true" class="font-bold">['r]</span>
		rustier playground
	</h1>
	<span class="text-on-surface-alt select-none" aria-hidden="true">|</span>
	<nav aria-label="examples" class="min-w-0">
		<DropdownMenu.Root>
			<DropdownMenu.Trigger>
				{#snippet child({ props })}
					<Button {...props} variant="foreground" color="white" class="max-w-full">
						<span class="truncate">Example: {current.name}</span>
					</Button>
				{/snippet}
			</DropdownMenu.Trigger>
			<DropdownMenu.Content aria-label="select an example" class="w-85 min-w-0">
				{#each groups as group (group)}
					<div role="group" aria-label={group} class="not-first:mt-2">
						<div aria-hidden="true" class="px-1 text-on-surface-alt">{group}</div>
						{#each examples.filter((example) => example.group === group) as example (example.slug)}
							<DropdownMenu.Item
								textValue={example.name}
								class={[
									'items-start pl-3 whitespace-normal',
									selected === example.slug && 'font-bold underline'
								]}
								aria-current={selected === example.slug ? 'page' : undefined}
								onSelect={() => goto(resolve('/playground/[[example]]', { example: example.slug }))}
							>
								{example.name}
							</DropdownMenu.Item>
						{/each}
					</div>
				{/each}
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	</nav>
</header>
