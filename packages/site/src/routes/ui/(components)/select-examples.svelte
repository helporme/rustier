<script lang="ts">
	import * as Select from '#lib/ui/components/select';

	let value = $state('');
</script>

<section aria-labelledby="selects" class="space-y-4">
	<h2 id="selects" class="font-brand text-2xl">Select</h2>
	<p class="text-ui text-on-ink-surface-variant">
		Arrow keys, typeahead, Enter, Escape, and click-away are supported.
	</p>
	<div class="flex flex-wrap gap-4">
		<Select.Root type="single" bind:value allowDeselect={false}>
			<Select.Trigger aria-label="Demo select" class="w-48">
				{value || 'Choose a language'}
			</Select.Trigger>
			<Select.Content>
				<Select.Group>
					<Select.GroupHeading class="px-3 py-1 text-ui font-bold text-on-ink-surface-variant">
						Languages
					</Select.GroupHeading>
					<Select.Item value="Rust" label="Rust" />
					<Select.Item value="TypeScript" label="TypeScript" />
				</Select.Group>
				<Select.Group>
					<Select.GroupHeading class="px-3 py-1 text-ui font-bold text-on-ink-surface-variant">
						Unavailable options
					</Select.GroupHeading>
					<Select.Item value="Unavailable" label="Unavailable" disabled />
				</Select.Group>
			</Select.Content>
		</Select.Root>
		<Select.Root type="single" disabled>
			<Select.Trigger aria-label="Disabled select">Disabled</Select.Trigger>
		</Select.Root>
		<Select.Root type="single">
			<Select.Trigger aria-label="Invalid select" aria-invalid="true"
				>Invalid selection</Select.Trigger
			>
			<Select.Content>
				<Select.Item value="valid" label="Choose a valid value" />
			</Select.Content>
		</Select.Root>
	</div>
	<p role="status" class="text-ui">Selected: {value || 'none'}</p>
</section>
