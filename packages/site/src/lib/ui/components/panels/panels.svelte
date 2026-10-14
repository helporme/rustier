<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cn } from '#lib/ui/utils';
	import ResizeHandle from './resize-handle.svelte';

	let {
		left,
		right,
		split = $bindable(0.5),
		class: className
	}: {
		left: Snippet;
		right: Snippet;
		split?: number;
		class?: string;
	} = $props();

	const id = $props.id();
	let root: HTMLDivElement;

	function resize(next: number) {
		const limit = Math.min(160 / root.clientWidth, 0.5);

		split =
			Math.abs(next - 0.5) * root.clientWidth <= 12
				? 0.5
				: Math.min(Math.max(next, limit), 1 - limit);
	}
</script>

<div
	bind:this={root}
	class={cn('relative grid h-full min-h-0 min-w-0', className)}
	style:grid-template-columns={`minmax(0, ${split}fr) minmax(0, ${1 - split}fr)`}
>
	<section id={`${id}-left`} aria-label="left pane" class="min-h-0 min-w-0">
		{@render left()}
	</section>
	<ResizeHandle
		label="Resize left and right panes"
		controls={`${id}-left`}
		value={split * 100}
		style={`left: ${split * 100}%`}
		onMove={(position) => {
			const bounds = root.getBoundingClientRect();

			resize((position - bounds.left) / bounds.width);
		}}
		onStep={(direction) => resize(split + direction * 0.05)}
		onEdge={(edge) => resize(edge === 'start' ? 0 : 1)}
	/>
	<section aria-label="right pane" class="min-h-0 min-w-0">
		{@render right()}
	</section>
</div>
