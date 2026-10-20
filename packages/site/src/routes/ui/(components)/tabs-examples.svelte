<script lang="ts">
	import * as Tabs from '#lib/ui/components/tabs';
	import { Toggle } from '#lib/ui/components/toggle';
	import { FileCode, Cpu, TriangleAlert } from '@lucide/svelte';

	let inspect = $state(false);
	let selected = $state('source');
</script>

<section aria-labelledby="tabs" class="space-y-4">
	<h2 id="tabs" class="font-brand text-2xl">Tabs</h2>
	<Toggle variant="alternative" bind:pressed={inspect}>Show diagnostics tab</Toggle>
	<Tabs.Root
		bind:value={
			() => (selected === 'diagnostics' && !inspect ? 'source' : selected),
			(value) => (selected = value)
		}
		class="flex h-40 flex-col overflow-hidden border border-ink-outline"
	>
		<Tabs.List aria-label="Example views">
			<Tabs.Trigger value="source"
				><FileCode aria-hidden="true" class="size-4" /> Source</Tabs.Trigger
			>
			<Tabs.Trigger value="output"><Cpu aria-hidden="true" class="size-4" /> Output</Tabs.Trigger>
			<Tabs.Trigger value="disabled" disabled>Disabled</Tabs.Trigger>
			{#if inspect}
				<Tabs.Trigger value="diagnostics">
					<TriangleAlert aria-hidden="true" class="size-4" /> Diagnostics [1]
				</Tabs.Trigger>
			{/if}
		</Tabs.List>
		<Tabs.Content value="source" class="p-4">Editable source files.</Tabs.Content>
		<Tabs.Content value="output" class="p-4">Read-only generated output.</Tabs.Content>
		<Tabs.Content value="diagnostics" class="p-4">Plain-text diagnostics.</Tabs.Content>
	</Tabs.Root>
</section>
