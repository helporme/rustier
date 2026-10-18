<script lang="ts">
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
		'relative flex min-h-8 cursor-default items-center gap-3 rounded-md py-1.5 pr-3 pl-8 outline-none select-none data-disabled:pointer-events-none data-disabled:opacity-50 data-highlighted:bg-primary data-highlighted:text-on-primary data-selected:font-bold',
		className
	)}
	{...restProps}
>
	{#snippet children({ selected, highlighted })}
		{#if selected}
			<svg
				aria-hidden="true"
				viewBox="0 0 16 16"
				class="absolute left-2 size-3 fill-none stroke-current stroke-2"
			>
				<path d="m3 8 3 3 7-7" />
			</svg>
		{/if}
		{#if childrenProp}
			{@render childrenProp({ selected, highlighted })}
		{:else}
			{label || value}
		{/if}
	{/snippet}
</SelectPrimitive.Item>
