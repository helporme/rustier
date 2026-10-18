<script lang="ts">
	import { Check } from '@lucide/svelte';
	import { Select as SelectPrimitive } from 'bits-ui';
	import { cn } from '#lib/ui/utils';

	let {
		ref = $bindable(null),
		class: className,
		value,
		label,
		disabled = false,
		children: childrenProp,
		...restProps
	}: Omit<SelectPrimitive.ItemProps, 'child'> = $props();
</script>

<SelectPrimitive.Item
	bind:ref
	{value}
	{label}
	{disabled}
	aria-disabled={disabled || undefined}
	data-slot="select-item"
	class={cn(
		'relative flex min-h-8 cursor-default items-center gap-3 rounded-none py-1.5 pr-3 pl-8 outline-none select-none data-disabled:pointer-events-none data-disabled:opacity-50 data-highlighted:bg-java data-highlighted:text-ink data-selected:font-bold',
		className
	)}
	{...restProps}
>
	{#snippet children({ selected, highlighted })}
		{#if selected}
			<Check aria-hidden="true" class="absolute left-2 size-3" />
		{/if}
		{#if childrenProp}
			{@render childrenProp({ selected, highlighted })}
		{:else}
			{label || value}
		{/if}
	{/snippet}
</SelectPrimitive.Item>
