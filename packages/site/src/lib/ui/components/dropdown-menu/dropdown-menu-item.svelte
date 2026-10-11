<script lang="ts">
	import { DropdownMenu as DropdownMenuPrimitive } from 'bits-ui';
	import { Button, type ButtonProps } from '#lib/ui/components/button';
	import { cn } from '#lib/ui/utils';

	let {
		ref = $bindable(null),
		color,
		class: className,
		children,
		...restProps
	}: Omit<DropdownMenuPrimitive.ItemProps, 'child'> & { color?: ButtonProps['color'] } = $props();
</script>

<DropdownMenuPrimitive.Item bind:ref data-slot="dropdown-menu-item" {...restProps}>
	{#snippet child({ props })}
		<Button {...props} {color} variant="ghost" class={cn('w-full ', className)}>
			{@render children?.()}
		</Button>
	{/snippet}
</DropdownMenuPrimitive.Item>
