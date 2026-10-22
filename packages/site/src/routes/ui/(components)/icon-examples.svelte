<script lang="ts" module>
	import {
		Logo,
		ViteIcon,
		TsdownIcon,
		WasmIcon,
		NodeIcon,
		GithubIcon
	} from '#lib/ui/components/icons';

	const icons = [
		{ name: 'Logo', component: Logo },
		{ name: 'ViteIcon', component: ViteIcon },
		{ name: 'TsdownIcon', component: TsdownIcon },
		{ name: 'WasmIcon', component: WasmIcon },
		{ name: 'NodeIcon', component: NodeIcon },
		{ name: 'GithubIcon', component: GithubIcon }
	];
</script>

<section aria-labelledby="icons" class="space-y-4">
	<h2 id="icons" class="font-brand text-2xl">Icons</h2>
	<code class="block overflow-x-auto font-code text-ui"
		>{'import { TsdownIcon, ViteIcon } from "#lib/ui/components/icons"'}</code
	>
	<div class="flex flex-wrap gap-6">
		{#each icons as { name, component: Icon } (name)}
			<div class="flex items-center gap-3">
				<Icon class="size-9" aria-hidden="true" />
				<span class="font-code text-ui">{name}</span>
			</div>
		{/each}
	</div>
	<p class="text-lg">
		Inline alignment:
		<ViteIcon /> Vite, <TsdownIcon /> tsdown, <WasmIcon /> WASM and <NodeIcon /> Node.js
	</p>
</section>
