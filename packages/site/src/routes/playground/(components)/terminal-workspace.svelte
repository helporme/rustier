<script lang="ts">
	import type { Snippet } from 'svelte';
	import * as Resizable from '#lib/ui/components/resizable';
	import { Button } from '#lib/ui/components/button';
	import type { Compilation } from '#lib/playground/compilation';
	import { MAX_TERMINAL_SIZE, type Terminal } from './terminal.svelte';
	import ExecutionOutput from './execution-output.svelte';

	let {
		terminal,
		compilation,
		children
	}: {
		terminal: Terminal;
		compilation: Compilation;
		children: Snippet;
	} = $props();

	const id = $props.id();
</script>

<div bind:clientHeight={terminal.groupHeight} class="h-full min-h-0 overflow-hidden">
	<Resizable.PaneGroup direction="vertical">
		<Resizable.Pane
			defaultSize={100}
			minSize={20}
			class={[
				'min-h-0',
				!terminal.dragging &&
					'motion-safe:transition-[flex-grow] motion-safe:duration-200 motion-safe:ease-out'
			]}
		>
			{@render children()}
		</Resizable.Pane>
		{#if compilation.status !== 'idle'}
			<div
				bind:clientHeight={terminal.handleHeight}
				style:height={`${terminal.handleSize}px`}
				class={[
					'relative z-10 flex shrink-0',
					!terminal.dragging &&
						'motion-safe:transition-[height] motion-safe:duration-150 motion-safe:ease-out'
				]}
			>
				<Button
					variant="ghost"
					aria-label={terminal.visible ? 'Resize terminal' : 'Open terminal'}
					aria-controls={id}
					aria-expanded={terminal.visible}
					data-active={terminal.dragging && terminal.compactHandle ? 'pointer' : undefined}
					onpointerdown={terminal.onPointerDown}
					onpointermove={terminal.onPointerMove}
					onpointerup={terminal.onPointerEnd}
					onpointercancel={terminal.onPointerEnd}
					onlostpointercapture={terminal.onPointerEnd}
					onclick={terminal.onClick}
					onkeydown={terminal.onKeyDown}
					class={[
						'group relative h-full w-full cursor-ns-resize touch-none border-0 p-0 outline-none after:absolute after:inset-x-0 after:-inset-y-3 focus-visible:ring-2 focus-visible:ring-rust focus-visible:outline-none data-[active=pointer]:bg-rust data-[active=pointer]:ring-1 data-[active=pointer]:ring-rust',
						terminal.compactHandle
							? 'bg-ink-outline hover:bg-ink-outline active:bg-ink-outline'
							: 'border-t border-ink-outline bg-ink-surface-bright hover:bg-ink-surface-bright active:bg-ink-surface-bright'
					]}
				>
					<span
						aria-hidden="true"
						style:opacity={terminal.gripVisibility}
						class={[
							'pointer-events-none h-[3px] w-10 rounded-full bg-on-ink-surface-variant/50 group-hover:bg-rust group-focus-visible:bg-rust',
							!terminal.dragging &&
								'motion-safe:transition-opacity motion-safe:duration-150 motion-safe:ease-out'
						]}
					></span>
				</Button>
			</div>
		{/if}
		<Resizable.Pane
			bind:this={terminal.pane}
			bind:ref={terminal.element}
			defaultSize={0}
			minSize={terminal.minSize}
			maxSize={MAX_TERMINAL_SIZE}
			collapsible
			collapsedSize={0}
			onResize={terminal.onResize}
			class={[
				'min-h-0',
				!terminal.dragging &&
					'motion-safe:transition-[flex-grow] motion-safe:duration-200 motion-safe:ease-out'
			]}
		>
			<div {id} class="h-full" inert={!terminal.visible}>
				{#if terminal.visible}
					<ExecutionOutput {compilation} />
				{/if}
			</div>
		</Resizable.Pane>
	</Resizable.PaneGroup>
</div>
