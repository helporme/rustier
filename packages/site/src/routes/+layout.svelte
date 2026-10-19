<script lang="ts">
	import './layout.css';
	import { setLucideProps } from '@lucide/svelte';
	import { ModeWatcher } from 'mode-watcher';
	import favicon from '#lib/assets/favicon.svg';
	import type { LayoutProps } from './$types';
	import { setupPageTransitions } from './(components)/page-transitions';

	let { children }: LayoutProps = $props();

	setLucideProps({ strokeWidth: 3 });
	setupPageTransitions();
</script>

<svelte:head>
	<link rel="icon" type="image/svg+xml" sizes="any" href={favicon} />
	<title>rustier</title>
</svelte:head>

<!-- Match --color-rust-surface in lib/ui/theme.css; browser metadata needs concrete colors. -->
<ModeWatcher modeStorageKey="rustier-theme" themeColors={{ light: '#f7ebe0', dark: '#3a302b' }} />
{@render children()}
