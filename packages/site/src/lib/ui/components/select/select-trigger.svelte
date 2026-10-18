<script lang="ts">
	import { Select as SelectPrimitive } from 'bits-ui';
	import { cn } from '#lib/ui/utils';

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: Omit<SelectPrimitive.TriggerProps, 'child'> = $props();
</script>

<SelectPrimitive.Trigger
	bind:ref
	data-slot="select-trigger"
	class={cn(
		'flex h-8.5 min-w-0 items-center justify-between gap-4 rounded-[9px] border border-ink-outline bg-ink-surface-bright px-3.5 font-body text-ui font-bold text-on-ink-surface outline-none select-none hover:border-primary-outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-secondary-outline focus-visible:outline-solid disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-secondary-outline data-placeholder:text-on-ink-surface-variant data-[state=open]:border-primary-outline',
		className
	)}
	{...restProps}
>
	<span class="truncate">{@render children?.()}</span>
	<svg
		aria-hidden="true"
		viewBox="0 0 16 16"
		class="size-3 shrink-0 fill-none stroke-current stroke-2"
	>
		<path d="m4 6 4 4 4-4" />
	</svg>
</SelectPrimitive.Trigger>
