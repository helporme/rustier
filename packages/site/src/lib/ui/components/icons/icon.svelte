<script lang="ts">
	import type { SVGAttributes } from 'svelte/elements';
	import { cn } from '#lib/ui/utils';

	let { class: className, children, ...props }: SVGAttributes<SVGSVGElement> = $props();
</script>

<svg
	xmlns="http://www.w3.org/2000/svg"
	width="1em"
	height="1em"
	fill="none"
	aria-hidden="true"
	focusable="false"
	{...props}
	class={cn('inline-block size-[1em] shrink-0 align-[-0.125em]', className)}
>
	{@render children?.()}
</svg>
