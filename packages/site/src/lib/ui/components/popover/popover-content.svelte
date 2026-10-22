<script lang="ts">
	import { Popover as PopoverPrimitive } from 'bits-ui';
	import { cn } from '#lib/ui/utils';

	let {
		ref = $bindable(null),
		class: className,
		sideOffset = 5,
		align = 'start',
		...restProps
	}: PopoverPrimitive.ContentProps = $props();
</script>

<PopoverPrimitive.Portal>
	<PopoverPrimitive.Content
		bind:ref
		data-slot="popover-content"
		{sideOffset}
		{align}
		class={cn(
			'z-50 max-h-(--bits-popover-content-available-height) w-80 max-w-(--bits-popover-content-available-width) overflow-y-auto rounded-none border border-rust-outline bg-rust-surface p-4 font-body text-ui text-on-ink-surface shadow-lg outline-none',
			className
		)}
		{...restProps}
	/>
</PopoverPrimitive.Portal>
