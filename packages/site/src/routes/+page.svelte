<script lang="ts">
	import { ArrowUpRight } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import { Button } from '#lib/ui/components/button';
	import { ViteIcon, TsdownIcon, WasmIcon, NodeIcon } from '#lib/ui/components/icons';
	import SiteHeader from './(components)/site-header.svelte';
	import GreetingDemo from './(components)/greeting-demo.svelte';
	import Waves from './(components)/waves.svelte';
</script>

<svelte:head>
	<title>JavaScript, but rustier</title>
	<meta
		name="description"
		content="Import Rust code in JavaScript. Rustier adapts ownership and borrowing to JavaScript’s object model, with Vite and tsdown plugins for WASM and native Node.js addons."
	/>
</svelte:head>

<div class="flex min-h-svh flex-col overflow-clip bg-rust-surface text-on-ink-surface">
	<SiteHeader />
	<main
		id="introduction"
		class="mx-auto grid w-full max-w-screen-2xl flex-1 items-start gap-10 px-6 py-8 lg:grid-cols-2 lg:gap-12 lg:px-18 lg:py-18"
	>
		<div class="min-w-0 space-y-6 sm:space-y-8">
			<h1
				class="font-brand text-5xl leading-none tracking-tight sm:text-6xl lg:text-5xl xl:text-7xl"
			>
				Making JavaScript <span class="block text-rust">rustier</span>
			</h1>
			<div class="space-y-5">
				<p class="text-base leading-relaxed sm:text-xl">
					<strong class="font-extrabold">Rustier lets you import Rust code in JavaScript,</strong>
					adapting ownership and borrowing to JavaScript’s object model.
				</p>
				<p class="text-base leading-8 sm:text-lg">
					Use it as a <a
						href="https://vite.dev"
						class="font-extrabold whitespace-nowrap hover:underline"
						><ViteIcon class="mr-1" />Vite</a
					>
					or
					<a href="https://tsdown.dev" class="font-extrabold whitespace-nowrap hover:underline"
						><TsdownIcon class="mr-1" />tsdown</a
					>
					plugin to target
					<span class="font-extrabold whitespace-nowrap"><WasmIcon class="mr-1" />WASM</span>
					or native
					<span class="font-extrabold whitespace-nowrap"
						><NodeIcon class="mr-1" />Node.js addons.</span
					>
				</p>
				<Button href={resolve('/playground')} size="lg"
					>Try the playground <ArrowUpRight aria-hidden="true" class="size-4" /></Button
				>
			</div>
		</div>
		<GreetingDemo />
	</main>
	<Waves />
</div>
