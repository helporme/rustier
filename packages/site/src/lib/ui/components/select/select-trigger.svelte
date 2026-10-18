<script lang="ts">
	import { ChevronDown } from '@lucide/svelte';
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
		'flex h-10 min-w-0 items-center justify-between gap-4 rounded-none border border-current bg-transparent px-3.5 font-body text-ui font-bold text-inherit outline-none select-none hover:border-rust focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-secondary-outline focus-visible:outline-solid disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-secondary-outline data-placeholder:text-on-ink-surface-variant data-[state=open]:border-rust',
		className
	)}
	{...restProps}
>
	<span class="truncate">{@render children?.()}</span>
	<ChevronDown aria-hidden="true" class="size-3 shrink-0" />
</SelectPrimitive.Trigger>
