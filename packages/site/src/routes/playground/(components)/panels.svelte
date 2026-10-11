<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cn } from '#lib/ui/utils';

	let {
		rust,
		typescript,
		results
	}: {
		rust: Snippet<[() => void]>;
		typescript: Snippet<[() => void]>;
		results: Snippet<[() => void]>;
	} = $props();

	let active = $state('rust');
	let secondary = $state('typescript');

	function selectPane(pane: 'rust' | 'typescript' | 'results') {
		active = pane;

		if (pane !== 'rust') secondary = pane;
	}
</script>

<div
	class="grid h-full min-h-0 min-w-0 grid-rows-[auto_1fr] [&_.code-editor]:[color-scheme:inherit]!"
>
	<div class="flex min-w-0 @min-[48rem]:grid @min-[48rem]:grid-cols-2 @min-[72rem]:grid-cols-3">
		<button
			class={cn(
				'w-fit cursor-pointer px-2 py-1 text-left @min-[48rem]:bg-surface',
				active === 'rust' ? 'bg-surface' : 'bg-transparent'
			)}
			aria-controls="rust-pane"
			onclick={() => selectPane('rust')}>lib.rs</button
		>
		<div class="flex min-w-0 @min-[72rem]:contents">
			<button
				class={cn(
					'w-fit cursor-pointer px-2 py-1 text-left @min-[72rem]:bg-surface-dim',
					active === 'typescript' ? 'bg-surface' : 'bg-transparent',
					secondary === 'typescript' ? '@min-[48rem]:bg-surface-dim' : '@min-[48rem]:bg-transparent'
				)}
				aria-controls="typescript-pane"
				onclick={() => selectPane('typescript')}>index.ts</button
			>
			<button
				class={cn(
					'w-fit cursor-pointer px-2 py-1 text-left @min-[72rem]:bg-surface-dimest @min-[72rem]:text-on-surface @min-[72rem]:scheme-dark',
					active === 'results' ? 'bg-surface' : 'bg-transparent',
					secondary === 'results' ? '@min-[48rem]:bg-surface-dim' : '@min-[48rem]:bg-transparent'
				)}
				aria-controls="results-pane"
				onclick={() => selectPane('results')}>Results</button
			>
		</div>
	</div>
	<div class="grid min-h-0 min-w-0 @min-[48rem]:grid-cols-2 @min-[72rem]:grid-cols-3">
		<section
			id="rust-pane"
			class={cn(
				'min-h-0 min-w-0 bg-surface [--editor-background:var(--color-surface)] @min-[48rem]:block',
				active === 'rust' ? 'block' : 'hidden'
			)}
			aria-label="lib.rs"
		>
			{@render rust(() => selectPane('rust'))}
		</section>
		<section
			id="typescript-pane"
			class={cn(
				'min-h-0 min-w-0 bg-(--editor-background) [--editor-background:var(--color-surface)] @min-[48rem]:[--editor-background:var(--color-surface-dim)] @min-[72rem]:block',
				active === 'typescript' ? 'block' : 'hidden',
				secondary === 'typescript' ? '@min-[48rem]:block' : '@min-[48rem]:hidden'
			)}
			aria-label="index.ts"
		>
			{@render typescript(() => selectPane('typescript'))}
		</section>
		<section
			id="results-pane"
			class={cn(
				'min-h-0 min-w-0 bg-(--editor-background) [--editor-background:var(--color-surface)] @min-[48rem]:[--editor-background:var(--color-surface-dim)] @min-[72rem]:block @min-[72rem]:scheme-dark @min-[72rem]:[--editor-background:var(--color-surface-dimest)]',
				active === 'results' ? 'block' : 'hidden',
				secondary === 'results' ? '@min-[48rem]:block' : '@min-[48rem]:hidden'
			)}
			aria-label="Results"
		>
			{@render results(() => selectPane('results'))}
		</section>
	</div>
</div>
