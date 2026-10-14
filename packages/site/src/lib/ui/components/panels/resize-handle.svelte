<script lang="ts">
	import { onMount } from 'svelte';

	let {
		label,
		controls,
		value,
		onMove,
		onStep,
		onEdge,
		style
	}: {
		label: string;
		controls: string;
		value: number;
		onMove: (position: number) => void;
		onStep: (direction: -1 | 1) => void;
		onEdge: (edge: 'start' | 'end') => void;
		style?: string;
	} = $props();

	let dragging = $state(false);
	let keyboardFocus = $state(false);
	let released = $state(false);
	let frame = 0;
	let position: number | undefined;

	onMount(() => () => cancelAnimationFrame(frame));

	function press(event: PointerEvent & { currentTarget: HTMLDivElement }) {
		if (event.button !== 0) return;

		event.preventDefault();
		keyboardFocus = false;
		released = false;
		event.currentTarget.setPointerCapture(event.pointerId);
		event.currentTarget.focus({ preventScroll: true });
		dragging = true;
	}

	function release() {
		cancelAnimationFrame(frame);
		frame = 0;
		if (position !== undefined) onMove(position);
		position = undefined;
		released = true;
		dragging = false;
	}

	function keydown(event: KeyboardEvent) {
		keyboardFocus = true;
		released = false;
		if (event.key === 'ArrowLeft') onStep(-1);
		else if (event.key === 'ArrowRight') onStep(1);
		else if (event.key === 'Home') onEdge('start');
		else if (event.key === 'End') onEdge('end');
		else return;

		event.preventDefault();
	}
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
	role="separator"
	tabindex="0"
	aria-label={label}
	aria-controls={controls}
	aria-orientation="vertical"
	aria-valuenow={Math.round(value)}
	aria-valuemin={0}
	aria-valuemax={100}
	data-dragging={dragging || undefined}
	data-keyboard-focus={keyboardFocus || undefined}
	data-released={released || undefined}
	{style}
	class="group/resize absolute inset-y-0 z-30 w-2 -translate-x-1/2 cursor-col-resize touch-none outline-none select-none pointer-coarse:w-6"
	onpointerdown={press}
	onpointerleave={() => (released = false)}
	onfocus={(event) => {
		keyboardFocus = event.currentTarget.matches(':focus-visible');
	}}
	onblur={() => (keyboardFocus = false)}
	onpointermove={(event) => {
		if (!dragging) return;

		position = event.clientX;
		if (frame) return;

		frame = requestAnimationFrame(() => {
			frame = 0;
			if (position !== undefined) onMove(position);
			position = undefined;
		});
	}}
	onpointerup={release}
	onpointercancel={release}
	onlostpointercapture={release}
	onkeydown={keydown}
>
	<div
		class="pointer-events-none absolute inset-y-0 left-1/2 w-1 -translate-x-1/2 bg-surface-brightest opacity-0 transition-opacity duration-200 group-hover/resize:opacity-100 group-data-dragging/resize:bg-magenta! group-data-dragging/resize:opacity-100! group-data-keyboard-focus/resize:bg-magenta group-data-keyboard-focus/resize:opacity-100 group-data-released/resize:opacity-0 motion-reduce:transition-none"
	></div>
</div>
