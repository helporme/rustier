<script lang="ts" module>
	import { type VariantProps, tv } from 'tailwind-variants';
	import { buttonVariants } from '#lib/ui/components/button';

	export const toggleVariants = tv({
		extend: buttonVariants,
		base: '[&_svg]:pointer-events-none [&_svg]:shrink-0',
		variants: {
			variant: {
				primary:
					'data-[state=off]:border-primary-outline data-[state=off]:bg-primary-surface data-[state=off]:text-on-primary-surface data-[state=off]:hover:bg-primary-surface-dim',
				alternative:
					'data-[state=off]:border-secondary-outline data-[state=off]:bg-secondary-surface data-[state=off]:text-on-secondary-surface data-[state=off]:hover:bg-secondary-surface-dim',
				outline: 'data-[state=on]:bg-secondary-surface data-[state=on]:text-on-secondary-surface',
				ghost: 'data-[state=on]:bg-secondary-surface data-[state=on]:text-on-secondary-surface'
			}
		},
		defaultVariants: { variant: 'primary', size: 'default' }
	});

	export type ToggleVariant = VariantProps<typeof toggleVariants>['variant'];
	export type ToggleSize = VariantProps<typeof toggleVariants>['size'];
	export type ToggleVariants = VariantProps<typeof toggleVariants>;
</script>

<script lang="ts">
	import { Toggle as TogglePrimitive } from 'bits-ui';
	import { cn } from '#lib/ui/utils';

	let {
		ref = $bindable(null),
		pressed = $bindable(false),
		class: className,
		size = 'default',
		variant = 'primary',
		...restProps
	}: TogglePrimitive.RootProps & {
		variant?: ToggleVariant;
		size?: ToggleSize;
	} = $props();
</script>

<TogglePrimitive.Root
	bind:ref
	bind:pressed
	data-slot="toggle"
	class={cn(toggleVariants({ variant, size }), className)}
	{...restProps}
/>
