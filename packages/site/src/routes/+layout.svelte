<script lang="ts">
	import '@fontsource/anonymous-pro/400.css';
	import '@fontsource/anonymous-pro/700.css';
	import './layout.css';
	import favicon from '#lib/assets/favicon.svg';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<link rel="icon" type="image/svg+xml" sizes="any" href={favicon} />
	<title>rustier</title>
</svelte:head>

{@render children()}
