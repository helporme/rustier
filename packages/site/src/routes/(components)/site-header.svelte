<script lang="ts">
	import { ArrowUpRight } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import { Logo } from '#lib/ui/components/icons';
	import { Button } from '#lib/ui/components/button';
	import { ThemeToggle } from '#lib/ui/components/theme-toggle';
</script>

<header
	class="mx-auto flex w-full max-w-screen-2xl flex-wrap items-center justify-between gap-4 px-6 py-5 lg:px-18"
>
	<a
		href={resolve('/')}
		aria-label="Rustier home"
		class="flex items-center gap-2 [view-transition-name:rustier-logo]"
	>
		<Logo class="size-9 sm:size-11" aria-hidden="true" />
		<span class="font-brand text-wordmark">rustier</span>
	</a>
	<nav aria-label="Main navigation" class="flex items-center gap-2 text-ui font-extrabold sm:gap-4">
		<a
			href="https://github.com/helporme/rustier"
			class="hidden hover:underline sm:inline"
			rel="external"
			>GitHub <ArrowUpRight aria-hidden="true" class="inline-block size-4 align-[-0.125em]" /></a
		>
		<Button href={resolve('/playground')} variant="outline">
			Playground <ArrowUpRight aria-hidden="true" class="size-4" />
		</Button>
		<ThemeToggle />
	</nav>
</header>
