<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		id,
		value,
		labelledby,
		ref = $bindable(),
		children
	}: {
		id: string;
		value: string;
		labelledby: string;
		ref?: HTMLDivElement;
		children: Snippet;
	} = $props();

	const helpId = $props.id();
</script>

<div
	bind:this={ref}
	{id}
	data-editor-pane={value}
	role="tabpanel"
	aria-labelledby={labelledby}
	aria-describedby={helpId}
	tabindex="0"
	class="relative min-h-0 min-w-0 flex-1 bg-inherit outline-none focus-visible:after:pointer-events-none focus-visible:after:absolute focus-visible:after:inset-0 focus-visible:after:z-100 focus-visible:after:border-2 focus-visible:after:border-magenta focus-visible:after:content-['']"
>
	<p id={helpId} class="sr-only">
		Press Enter to focus the code editor. Press Escape then Tab or Shift+Tab to move focus out.
	</p>
	{@render children()}
</div>
