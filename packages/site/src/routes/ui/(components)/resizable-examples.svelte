<script lang="ts" module>
	import * as Resizable from '#lib/ui/components/resizable';

	const directions = ['horizontal', 'vertical'] as const;
</script>

<script lang="ts">
	const examples = directions.map((direction) => ({ direction, snap: new Resizable.CenterSnap() }));
</script>

<section aria-labelledby="resizable" class="space-y-4">
	<h2 id="resizable" class="font-brand text-2xl">Resizable panes</h2>
	<p class="text-ui text-on-ink-surface-variant">
		Drag the divider with a mouse or touch. It snaps to an equal split within 8 pixels of the
		center. Focus it and use the arrow keys, Home, or End.
	</p>
	<div class="grid gap-6 md:grid-cols-2">
		{#each examples as { direction, snap } (direction)}
			<div class="h-64 overflow-hidden rounded-lg border border-ink-outline">
				<Resizable.PaneGroup
					bind:ref={snap.group}
					{direction}
					keyboardResizeBy={5}
					onLayoutChange={snap.onLayoutChange}
				>
					<Resizable.Pane bind:this={snap.pane} defaultSize={50} minSize={20}>
						<div class="flex h-full items-center justify-center bg-ink-surface text-ui">
							First pane
						</div>
					</Resizable.Pane>
					<Resizable.Handle
						aria-label={`Resize ${direction} example`}
						aria-orientation={direction === 'horizontal' ? 'vertical' : 'horizontal'}
					/>
					<Resizable.Pane defaultSize={50} minSize={20}>
						<div class="flex h-full items-center justify-center bg-ink-surface-bright text-ui">
							Second pane
						</div>
					</Resizable.Pane>
				</Resizable.PaneGroup>
			</div>
		{/each}
	</div>
</section>
