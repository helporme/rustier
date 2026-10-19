<script lang="ts">
	import { Tabs as TabsPrimitive } from 'bits-ui';
	import { cn } from '#lib/ui/utils';

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: Omit<TabsPrimitive.TriggerProps, 'child'> = $props();
</script>

<TabsPrimitive.Trigger
	bind:ref
	data-slot="tabs-trigger"
	class={cn(
		'inline-flex h-9 shrink-0 items-center justify-center gap-2 border-r border-b border-ink-outline px-4 font-code text-ui whitespace-nowrap text-on-ink-surface-variant outline-none hover:bg-ink-surface-bright focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-secondary-outline disabled:pointer-events-none disabled:opacity-50 data-[state=active]:border-b-transparent data-[state=active]:bg-ink-surface-bright data-[state=active]:text-on-ink-surface',
		className
	)}
	{...restProps}
>
	{#snippet child({ props })}
		<button
			{...props}
		>
			{@render children?.()}
		</button>
	{/snippet}
</TabsPrimitive.Trigger>
