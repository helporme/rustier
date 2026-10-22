<script lang="ts">
	import { Button } from '#lib/ui/components/button';
	import { Input } from '#lib/ui/components/input';
	import * as Popover from '#lib/ui/components/popover';

	const id = $props.id();
	let title = $state('');
</script>

<section aria-labelledby="popovers" class="space-y-4">
	<h2 id="popovers" class="font-brand text-2xl">Popover and text input</h2>
	<Popover.Root>
		<Popover.Trigger>
			{#snippet child({ props })}
				<Button {...props} variant="outline">Open popover</Button>
			{/snippet}
		</Popover.Trigger>
		<Popover.Content aria-labelledby={`${id}-heading`}>
			<div class="space-y-4">
				<h3 id={`${id}-heading`} class="font-bold">Popover heading</h3>
				<div class="space-y-2">
					<label for={`${id}-input`}>Text input</label>
					<Input id={`${id}-input`} bind:value={title} placeholder="Enter some text" />
				</div>
				<Popover.Close>
					{#snippet child({ props })}
						<Button {...props}>Done</Button>
					{/snippet}
				</Popover.Close>
			</div>
		</Popover.Content>
	</Popover.Root>
	<Input aria-label="Disabled text input" value="Disabled" disabled />
</section>
