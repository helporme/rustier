<script lang="ts">
	import type { SVGAttributes } from 'svelte/elements';
	import { cn } from '#lib/ui/utils';
	import Icon from './icon.svelte';

	let props: SVGAttributes<SVGSVGElement> = $props();
</script>

<Icon
	viewBox="12 10 76 82"
	data-icon="tsdown"
	{...props}
	class={cn('size-[1.125em] align-[-0.2em]', props.class)}
>
	<!-- Frying-pan mark from rolldown/tsdown, docs/public/tsdown.svg. -->
	<path
		d="M28.936 21.766c6.044-6.801 24.17-9.271 28.82-6.88 4.65 2.39 18.957 20.6 16.27 25.07-4.494 7.474-12.146 12.953-13.56 14.04-1.63-2.596-11.845 1.812-9.51 6.5.177 1.968-9.958 6.737-14.571 7.132-.39.096-.753.1-1.08-.002-6.006-.736-16.748-13.029-18.04-18.01-2.307-8.906 9.442-25.343 11.67-27.85"
		class="fill-on-ink-surface-variant stroke-ink"
		stroke-width="2"
	/>
	<path
		d="M50.973 60.471c-1.944-4.87 7.645-9.002 9.5-6.5l21.49 23.02c-1.345-2.42-13.998 4.749-11.5 9z"
		class="fill-rust stroke-ink"
		stroke-width="2"
	/>
	<path
		d="M39.002 25.702c-1.339-3.997 17.228-6.866 17.252-5.261 2.252.369 11.882 14.597 10.554 14.38.063 2.62-12.309 8.091-14.769 7.032-1.408 0-10.567-8.776-13.037-16.15Z"
		class="fill-java stroke-ink"
		stroke-width="2"
	/>
	<path
		d="M33.883 39.842c1.183-1.285 5.424 3.831 4.557 5.355-1.38 2.425-5.398 5.828-5.398 6.842s2.017 1.64 2.017 3.517c0 .994-8.02.088-8.004-.505-.627-.46-1.84-6.379-1.604-9.43.235-3.051 3.255 2.765 3.737 2.001 2.88-4.562 3.513-6.494 4.695-7.78Z"
		class="fill-java"
	/>
</Icon>
