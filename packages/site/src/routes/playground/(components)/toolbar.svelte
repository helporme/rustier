<script lang="ts">
	import { Microscope, Play } from '@lucide/svelte';
	import { Logo } from '#lib/ui/components/icons';
	import { Button } from '#lib/ui/components/button';
	import { Toggle } from '#lib/ui/components/toggle';
	import * as Select from '#lib/ui/components/select';
	import { resolve } from '$app/paths';
	import { examples, exampleGroups } from './examples';

	let {
		selected = $bindable(),
		inspect = $bindable(false),
		pending = false,
		onrun
	}: {
		selected: string;
		inspect: boolean;
		pending?: boolean;
		onrun: () => void;
	} = $props();

	const current = $derived(examples.find((example) => example.id === selected));
</script>

<header
	class="flex min-h-15 shrink-0 items-center justify-between gap-2 border-b border-ink-outline bg-rust-surface px-3 py-2 text-on-ink-surface sm:gap-4 sm:px-6"
>
	<div class="flex min-w-0 flex-1 items-center gap-2 sm:gap-4">
		<h1 class="flex shrink-0 items-center gap-3">
			<a href={resolve('/')} aria-label="Rustier home" class="flex items-center gap-2">
				<Logo class="size-10 [view-transition-name:rustier-logo]" aria-hidden="true" />
				<span
					class="hidden font-brand text-wordmark [view-transition-name:rustier-wordmark] sm:inline"
					>rustier</span
				>
			</a>
			<span class="sr-only">playground</span>
		</h1>
		<span id="example-label" class="sr-only">Example</span>
		<Select.Root type="single" bind:value={selected} allowDeselect={false}>
			<Select.Trigger aria-labelledby="example-label" class="w-52 max-w-full shrink px-2 sm:px-3.5">
				{current?.name ?? 'Choose an example'}
			</Select.Trigger>
			<Select.Content>
				{#each exampleGroups as group (group.topic)}
					<Select.Group class="not-first:mt-2 not-first:border-t not-first:border-ink-outline">
						<Select.GroupHeading
							class="px-3 pt-2 pb-1 text-ui font-bold text-on-ink-surface-variant"
						>
							{group.topic}
						</Select.GroupHeading>
						{#each group.examples as example (example.id)}
							<Select.Item value={example.id} label={example.name} />
						{/each}
					</Select.Group>
				{/each}
			</Select.Content>
		</Select.Root>
	</div>
	<div class="flex shrink-0 items-center gap-2">
		<Toggle
			variant="alternative"
			bind:pressed={inspect}
			aria-label="Inspect"
			class="w-10 px-0 min-[30rem]:w-auto min-[30rem]:px-3 sm:px-5"
		>
			<Microscope aria-hidden="true" class="size-4" />
			<span class="hidden min-[30rem]:inline">Inspect</span>
		</Toggle>
		<Button
			onclick={onrun}
			disabled={pending}
			aria-busy={pending}
			aria-label={pending ? 'Running…' : 'Run'}
			class="w-10 px-0 min-[30rem]:w-auto min-[30rem]:px-5"
		>
			<Play aria-hidden="true" class="size-3 fill-current" />
			<span class="hidden min-[30rem]:inline">{pending ? 'Running…' : 'Run'}</span>
		</Button>
	</div>
</header>
