<script lang="ts">
	import { Logo } from '#lib/ui/components/logo';
	import { Button } from '#lib/ui/components/button';
	import * as Select from '#lib/ui/components/select';
	import { examples } from './examples';

	let {
		selected = $bindable(),
		onrun
	}: {
		selected: string;
		onrun: () => void;
	} = $props();

	const current = $derived(examples.find((example) => example.id === selected));
</script>

<header
	class="flex min-h-15 shrink-0 flex-wrap items-center justify-between gap-x-6 gap-y-1 border-b border-ink-outline bg-primary-surface-dim px-6 py-2 text-on-primary-surface-dim md:grid md:grid-cols-2 md:gap-0 md:p-0"
>
	<h1 class="flex items-center gap-3 md:px-6">
		<Logo class="size-10" aria-hidden="true" />
		<span class="font-brand text-wordmark">Rustier</span>
		<span class="sr-only">playground</span>
	</h1>
	<div
		class="flex min-w-0 flex-wrap items-center gap-3.5 md:justify-end md:self-stretch md:border-l md:border-ink-outline md:px-6 md:py-2"
	>
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
		<Button onclick={onrun} title="Compiler integration is not connected yet">
			<svg aria-hidden="true" viewBox="0 0 12 12" class="size-2.5 fill-current">
				<path d="M3 1v10l8-5Z" />
			</svg>
			Run
		</Button>
	</div>
</header>
