<script lang="ts">
	import { Tabs as TabsPrimitive } from 'bits-ui';
	import { cn } from '#lib/ui/utils';

	let { ref = $bindable(null), class: className, ...restProps }: TabsPrimitive.ListProps = $props();
</script>

<TabsPrimitive.List
	bind:ref
	data-slot="tabs-list"
	class={cn(
		'flex h-9 min-w-0 shrink-0 [scrollbar-width:none] items-stretch overflow-x-auto overflow-y-hidden bg-ink-surface after:flex-1 after:border-b after:border-ink-outline [&::-webkit-scrollbar]:hidden',
		className
	)}
	{...restProps}
/>
