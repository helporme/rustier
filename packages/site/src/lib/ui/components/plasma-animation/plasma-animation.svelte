<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import { cn } from '#lib/ui/utils';
	import { PlasmaRenderer } from './renderer';

	let {
		busy,
		class: className,
		children
	}: {
		busy: boolean;
		class?: string;
		children: Snippet;
	} = $props();

	let surface: HTMLDivElement;
	let canvas: HTMLCanvasElement;
	let swatches: HTMLDivElement;
	let field: PlasmaRenderer | undefined;
	let mounted = $state(false);
	let animating = $state(false);
	let reducedMotion = $state(false);
	let supported = $state(true);

	onMount(() => {
		field = PlasmaRenderer.create(canvas, swatches, (running) => {
			animating = running;
		});
		supported = !!field;

		let disposed = false;
		const bounds = surface.getBoundingClientRect();
		let inView =
			bounds.width > 0 &&
			bounds.height > 0 &&
			bounds.bottom > 0 &&
			bounds.right > 0 &&
			bounds.top < innerHeight &&
			bounds.left < innerWidth;
		let awake = document.visibilityState === 'visible';
		field?.resize(bounds.width, bounds.height);
		const motion = matchMedia('(prefers-reduced-motion: reduce)');
		const scheme = matchMedia('(prefers-color-scheme: dark)');
		const invalidate = () => {
			if (!disposed) field?.invalidatePalette();
		};
		const updateMotion = () => {
			reducedMotion = motion.matches;
		};
		const updateVisibility = () => {
			awake = document.visibilityState === 'visible';
			field?.setVisible(inView && awake);
		};
		const intersection = new IntersectionObserver(([entry]) => {
			inView = entry.isIntersecting;
			field?.setVisible(inView && awake);
		});
		const resize = new ResizeObserver(([entry]) => {
			field?.resize(entry.contentRect.width, entry.contentRect.height);
		});
		const theme = new MutationObserver(invalidate);

		for (let ancestor: HTMLElement | null = surface; ancestor; ancestor = ancestor.parentElement) {
			theme.observe(ancestor, { attributes: true, attributeFilter: ['class', 'style'] });
		}

		updateMotion();
		updateVisibility();
		motion.addEventListener('change', updateMotion);
		scheme.addEventListener('change', invalidate);
		document.addEventListener('visibilitychange', updateVisibility);
		document.fonts.addEventListener('loadingdone', invalidate);
		void document.fonts.ready.then(invalidate);
		intersection.observe(surface);
		resize.observe(surface);
		mounted = true;

		return () => {
			disposed = true;
			motion.removeEventListener('change', updateMotion);
			scheme.removeEventListener('change', invalidate);
			document.removeEventListener('visibilitychange', updateVisibility);
			document.fonts.removeEventListener('loadingdone', invalidate);
			intersection.disconnect();
			resize.disconnect();
			theme.disconnect();
			field?.destroy();
		};
	});

	$effect(() => {
		if (mounted) field?.update(busy, reducedMotion);
	});
</script>

<div
	bind:this={surface}
	aria-busy={busy}
	data-plasma-animation
	class={cn('relative h-full min-h-0 min-w-0', className)}
>
	{@render children()}
	<div aria-hidden="true" class="pointer-events-none absolute inset-0 z-20 select-none">
		<canvas
			bind:this={canvas}
			data-plasma-canvas
			class="block h-full w-full font-mono"
			class:hidden={reducedMotion || !supported || !animating}
		></canvas>
		<div bind:this={swatches} class="hidden">
			<span class="bg-primary-container text-on-primary-container"></span>
			<span class="bg-primary text-on-primary"></span>
			<span class="bg-blue text-on-blue"></span>
			<span class="bg-cyan text-on-cyan"></span>
			<span class="bg-cyan-container text-on-cyan-container"></span>
		</div>
		{#if busy && (reducedMotion || !supported)}
			<span class="absolute top-2 right-2 text-primary">[ . . . ]</span>
		{/if}
	</div>
</div>
