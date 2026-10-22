<script lang="ts">
	import { ArrowUpRight } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import { Logo, GithubIcon } from '#lib/ui/components/icons';
	import { Button } from '#lib/ui/components/button';
	import { ThemeToggle } from '#lib/ui/components/theme-toggle';
</script>

<header
	class="mx-auto flex w-full max-w-screen-2xl flex-wrap items-center justify-between gap-4 px-6 py-5 lg:px-18"
>
	<a href={resolve('/')} aria-label="Rustier home" class="flex items-center gap-2">
		<Logo class="size-9 [view-transition-name:rustier-logo] sm:size-11" aria-hidden="true" />
		<span class="font-brand text-wordmark [view-transition-name:rustier-wordmark]">rustier</span>
	</a>
	<nav aria-label="Main navigation" class="flex items-center gap-2 text-ui font-extrabold sm:gap-4">
		<ThemeToggle />
		<a
			href="https://github.com/helporme/rustier"
			aria-label="GitHub"
			class="inline-flex h-8 min-w-8 items-center justify-center gap-2 hover:underline"
			rel="external"
		>
			<GithubIcon aria-hidden="true" class="size-5 sm:size-4" />
			<span class="hidden sm:inline">GitHub</span>
			<ArrowUpRight aria-hidden="true" class="hidden size-4 sm:block" />
		</a>
		<Button
			href={resolve('/playground/[[slug]]', {})}
			variant="outline"
			class="hidden md:inline-flex"
		>
			Playground <ArrowUpRight aria-hidden="true" class="size-4" />
		</Button>
	</nav>
</header>
