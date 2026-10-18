<script lang="ts" module>
	import { tv, type VariantProps } from 'tailwind-variants';
	import { cn, type WithElementRef } from '#lib/ui/utils';
	import type { HTMLAnchorAttributes, HTMLButtonAttributes } from 'svelte/elements';

	export const buttonVariants = tv({
		base: 'inline-flex shrink-0 items-center justify-center gap-2 rounded-none border-2 border-transparent font-body text-ui font-extrabold whitespace-nowrap outline-none select-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-solid focus-visible:outline-secondary-outline disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50',
		variants: {
			variant: {
				default: 'border-ink bg-java text-ink hover:bg-java/80 active:bg-java/65',
				outline:
					'border-current bg-transparent text-inherit hover:bg-current/10 active:bg-current/15',
				ghost: 'text-on-ink-surface hover:bg-ink-surface active:bg-ink-surface/70'
			},
			size: {
				default: 'h-10 px-5',
				lg: 'h-11 px-5 text-base',
				icon: 'size-8'
			}
		},
		defaultVariants: { variant: 'default', size: 'default' }
	});

	export type ButtonVariant = VariantProps<typeof buttonVariants>['variant'];
	export type ButtonSize = VariantProps<typeof buttonVariants>['size'];
	export type ButtonProps = WithElementRef<HTMLButtonAttributes & HTMLAnchorAttributes> & {
		variant?: ButtonVariant;
		size?: ButtonSize;
		loading?: boolean;
	};
</script>

<script lang="ts">
	import { LoaderCircle } from '@lucide/svelte';

	let {
		class: className,
		variant = 'default',
		size = 'default',
		ref = $bindable(null),
		href,
		type = 'button',
		disabled,
		loading = false,
		children,
		...restProps
	}: ButtonProps = $props();

	function setRef(element: HTMLElement) {
		ref = element;
		return () => {
			if (ref === element) ref = null;
		};
	}
</script>

{#snippet content()}
	{#if loading}
		<LoaderCircle aria-hidden="true" class="size-3 animate-spin motion-reduce:animate-none" />
	{/if}
	{@render children?.()}
{/snippet}

{#if href}
	<a
		{@attach setRef}
		data-slot="button"
		class={cn(buttonVariants({ variant, size }), className)}
		href={disabled || loading ? undefined : href}
		aria-disabled={disabled || loading || undefined}
		aria-busy={loading || undefined}
		role={disabled || loading ? 'link' : undefined}
		tabindex={disabled || loading ? -1 : undefined}
		{...restProps}
	>
		{@render content()}
	</a>
{:else}
	<button
		{@attach setRef}
		data-slot="button"
		class={cn(buttonVariants({ variant, size }), className)}
		{type}
		disabled={disabled || loading}
		aria-busy={loading || undefined}
		{...restProps}
	>
		{@render content()}
	</button>
{/if}
