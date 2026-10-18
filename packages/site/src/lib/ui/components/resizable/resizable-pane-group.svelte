<script lang="ts">
	import { PaneGroup as PaneGroupPrimitive, type PaneGroupProps } from 'paneforge';
	import { cn } from '#lib/ui/utils';

	let { ref = $bindable(null), class: className, ...restProps }: PaneGroupProps = $props();
</script>

<PaneGroupPrimitive
	bind:ref
	data-slot="resizable-pane-group"
	class={cn('flex h-full min-h-0 w-full min-w-0 data-[direction=vertical]:flex-col', className)}
	{...restProps}
/>
