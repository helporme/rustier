<script lang="ts">
	import { Play } from '@lucide/svelte';
	import { Logo } from '#lib/ui/components/icons';
	import { Button } from '#lib/ui/components/button';
	import { Toggle } from '#lib/ui/components/toggle';
	import * as Select from '#lib/ui/components/select';
	import { resolve } from '$app/paths';
	import { examples } from './examples';

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
	class="flex min-h-15 shrink-0 flex-wrap items-center justify-between gap-x-6 gap-y-2 border-b border-ink-outline bg-rust-surface px-6 py-2 text-on-ink-surface"
>
	<div class="flex min-w-0 flex-wrap items-center gap-x-6 gap-y-2">
		<h1 class="flex items-center gap-3">
			<a
				href={resolve('/')}
				aria-label="Rustier home"
				class="flex items-center gap-2 [view-transition-name:rustier-logo]"
			>
				<Logo class="size-10" aria-hidden="true" />
				<span class="font-brand text-wordmark">rustier</span>
			</a>
			<span class="sr-only">playground</span>
		</h1>
		<span id="example-label" class="sr-only">Example</span>
		<Select.Root type="single" bind:value={selected} allowDeselect={false}>
			<Select.Trigger aria-labelledby="example-label" class="w-52">
				{current?.name ?? 'Choose an example'}
			</Select.Trigger>
			<Select.Content>
				{#each examples as example (example.id)}
					<Select.Item value={example.id} label={example.name} />
				{/each}
			</Select.Content>
		</Select.Root>
	</div>
	<div class="flex items-center gap-3.5">
		<Toggle variant="alternative" bind:pressed={inspect}>Inspect</Toggle>
		<Button onclick={onrun} disabled={pending} aria-busy={pending}>
			<Play aria-hidden="true" class="size-3 fill-current" />
			{pending ? 'Running…' : 'Run'}
		</Button>
	</div>
</header>
