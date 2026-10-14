<script lang="ts" module>
	import { cn, type WithElementRef } from '#lib/ui/utils';
	import type { HTMLAnchorAttributes, HTMLButtonAttributes } from 'svelte/elements';
	import { tv, type VariantProps } from 'tailwind-variants';

	export const buttonVariants = tv({
		base: 'group/button cursor-pointer inline-flex shrink-0 items-center justify-start rounded-none px-1 text-left font-normal whitespace-nowrap text-on-surface outline-none select-none hover:bg-selection focus-visible:bg-selection focus-visible:outline-2 focus-visible:outline-solid focus-visible:-outline-offset-2 focus-visible:outline-magenta data-highlighted:bg-selection disabled:pointer-events-none disabled:opacity-50',
		variants: {
			variant: {
				background: '',
				foreground: ''
			},
			color: {
				white: 'text-on-surface',
				black: 'text-on-surface',
				red: 'text-red',
				green: 'text-green',
				yellow: 'text-yellow',
				blue: 'text-blue',
				magenta: 'text-magenta',
				cyan: 'text-cyan'
			}
		},
		defaultVariants: {
			variant: 'background',
			color: 'white'
		},
		compoundVariants: [
			{
				variant: 'foreground',
				color: 'white',
				className: 'bg-surface-bright text-on-surface'
			},
			{
				variant: 'foreground',
				color: 'black',
				className:
					'scheme-dark bg-surface-dimest text-on-surface hover:bg-surface-dim focus-visible:bg-surface-dim data-highlighted:bg-surface-dim'
			},
			{
				variant: 'foreground',
				color: 'red',
				className:
					'bg-red-container text-on-red-container hover:bg-red hover:text-on-red focus-visible:bg-red focus-visible:text-on-red'
			},
			{
				variant: 'foreground',
				color: 'green',
				className:
					'bg-green-container text-on-green-container hover:bg-green hover:text-on-green focus-visible:bg-green focus-visible:text-on-green'
			},
			{
				variant: 'foreground',
				color: 'yellow',
				className:
					'bg-yellow-container text-on-yellow-container hover:bg-yellow hover:text-on-yellow focus-visible:bg-yellow focus-visible:text-on-yellow'
			},
			{
				variant: 'foreground',
				color: 'blue',
				className:
					'bg-blue-container text-on-blue-container hover:bg-blue hover:text-on-blue focus-visible:bg-blue focus-visible:text-on-blue'
			},
			{
				variant: 'foreground',
				color: 'magenta',
				className:
					'bg-magenta-container text-on-magenta-container hover:bg-magenta hover:text-on-magenta focus-visible:bg-magenta focus-visible:text-on-magenta'
			},
			{
				variant: 'foreground',
				color: 'cyan',
				className:
					'bg-cyan-container text-on-cyan-container hover:bg-cyan hover:text-on-cyan focus-visible:bg-cyan focus-visible:text-on-cyan'
			}
		]
	});

	export type ButtonProps = WithElementRef<
		Omit<HTMLButtonAttributes & HTMLAnchorAttributes, 'prefix'>,
		HTMLButtonElement | HTMLAnchorElement
	> & {
		variant?: NonNullable<VariantProps<typeof buttonVariants>['variant']>;
		color?: NonNullable<VariantProps<typeof buttonVariants>['color']>;
		prefix?: boolean;
	};
</script>

<script lang="ts">
	let {
		class: className,
		variant = 'background',
		color = 'white',
		prefix = false,
		ref = $bindable(null),
		type = 'button',
		disabled,
		href,
		children,
		...restProps
	}: ButtonProps = $props();
</script>

{#snippet content()}
	{#if prefix}
		<span
			aria-hidden="true"
			class="hidden w-[2ch] shrink-0 group-hover/button:block group-focus-visible/button:block group-data-highlighted/button:block"
			>></span
		>
	{/if}
	{@render children?.()}
{/snippet}

{#if href !== undefined}
	<a
		bind:this={ref}
		data-slot="button"
		class={buttonVariants({ variant, color, class: cn(className) })}
		href={disabled ? undefined : href}
		{...restProps}
		aria-disabled={disabled || undefined}
		tabindex={disabled ? -1 : restProps.tabindex}
		onclick={(event) => {
			if (disabled) {
				event.preventDefault();
				event.stopImmediatePropagation();
				return;
			}

			restProps.onclick?.(event);
		}}
	>
		{@render content()}
	</a>
{:else}
	<button
		bind:this={ref}
		data-slot="button"
		class={buttonVariants({ variant, color, class: cn(className) })}
		{type}
		{disabled}
		{...restProps}
	>
		{@render content()}
	</button>
{/if}
