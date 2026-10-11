<script lang="ts">
	import { DropdownMenu as DropdownMenuPrimitive } from 'bits-ui';
	import { cn } from '#lib/ui/utils';
	import DropdownMenuPortal from './dropdown-menu-portal.svelte';

	let {
		ref = $bindable(null),
		sideOffset = 4,
		align = 'start',
		portalProps,
		class: className,
		...restProps
	}: DropdownMenuPrimitive.ContentProps & {
		portalProps?: Omit<DropdownMenuPrimitive.PortalProps, 'children'>;
	} = $props();
</script>

<DropdownMenuPortal {...portalProps}>
	<DropdownMenuPrimitive.Content
		bind:ref
		data-slot="dropdown-menu-content"
		{sideOffset}
		{align}
		class={cn(
			'dark z-50 flex max-h-(--bits-dropdown-menu-content-available-height) w-max max-w-[calc(100vw-1rem)] min-w-(--bits-dropdown-menu-anchor-width) flex-col overflow-y-auto bg-surface-dim p-1 text-on-surface outline-none',
			className
		)}
		{...restProps}
	/>
</DropdownMenuPortal>
