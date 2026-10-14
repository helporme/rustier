<script lang="ts" module>
	import { Toggle as TogglePrimitive } from 'bits-ui';
	import { buttonVariants, type ButtonProps } from '#lib/ui/components/button';
	import { cn } from '#lib/ui/utils';

	export type ToggleVariant = NonNullable<ButtonProps['variant']>;
	export type ToggleColor = NonNullable<ButtonProps['color']>;
	export type ToggleProps = TogglePrimitive.RootProps & {
		variant?: ToggleVariant;
		color?: ToggleColor;
	};

	export function toggleVariants({
		variant = 'background',
		color = 'white',
		pressed = false,
		class: className
	}: Pick<ToggleProps, 'variant' | 'color' | 'pressed' | 'class'> = {}) {
		const foreground = variant === 'foreground' || pressed;

		return buttonVariants({
			variant: foreground ? 'foreground' : 'background',
			color,
			class: cn('group/toggle', className)
		});
	}
</script>

<script lang="ts">
	let {
		ref = $bindable(null),
		pressed = $bindable(false),
		variant = 'background',
		color = 'white',
		class: className,
		...restProps
	}: ToggleProps = $props();
</script>

<TogglePrimitive.Root
	bind:ref
	bind:pressed
	data-slot="toggle"
	class={toggleVariants({ variant, color, pressed, class: className })}
	{...restProps}
/>
