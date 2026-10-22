<script lang="ts">
	import { cn, type WithElementRef } from '#lib/ui/utils';
	import type { HTMLInputAttributes } from 'svelte/elements';

	let {
		ref = $bindable(null),
		value = $bindable(''),
		class: className,
		...restProps
	}: WithElementRef<Omit<HTMLInputAttributes, 'type'>> = $props();

	function setRef(element: HTMLInputElement) {
		ref = element;
		return () => {
			if (ref === element) ref = null;
		};
	}
</script>

<input
	{@attach setRef}
	bind:value
	type="text"
	data-slot="input"
	class={cn(
		'h-10 w-full min-w-0 rounded-none border border-rust-outline bg-rust-surface px-3 font-body text-ui text-on-ink-surface outline-none placeholder:text-on-ink-surface-variant focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-secondary-outline disabled:pointer-events-none disabled:opacity-50',
		className
	)}
	{...restProps}
/>
