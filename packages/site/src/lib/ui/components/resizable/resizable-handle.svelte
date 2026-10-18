<script lang="ts">
	import { PaneResizer, type PaneResizerProps } from 'paneforge';
	import { cn } from '#lib/ui/utils';

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	}: Omit<PaneResizerProps, 'child' | 'children'> = $props();
</script>

<PaneResizer
	bind:ref
	data-slot="resizable-handle"
	class={cn(
		'relative z-10 h-full w-px shrink-0 bg-ink-outline outline-none after:absolute after:-inset-3 focus-visible:ring-2 focus-visible:ring-ink-outline data-[active=pointer]:ring-1 data-[active=pointer]:ring-ink-outline data-[direction=vertical]:h-px data-[direction=vertical]:w-full data-[enabled=false]:pointer-events-none data-[enabled=false]:opacity-50 pointer-coarse:after:-inset-6',
		className
	)}
	{...restProps}
/>
