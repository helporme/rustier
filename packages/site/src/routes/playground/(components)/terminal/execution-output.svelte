<script lang="ts" module>
	const runningMessage = 'Running...\n';
</script>

<script lang="ts">
	import type { Compilation } from '#lib/playground/compilation';

	let { compilation }: { compilation: Compilation } = $props();
</script>

<section
	aria-label="Run output"
	class="flex h-full min-h-0 flex-col bg-ink-surface-bright text-on-ink-surface"
>
	<pre
		role="log"
		aria-live="polite"
		class="min-h-0 flex-1 overflow-auto px-6 pt-3 pb-4 font-mono text-code whitespace-pre-wrap">{#if compilation.pending}{runningMessage}{/if}{#if compilation.output}{compilation.output}{:else if !compilation.pending}<span
				class="text-on-ink-surface-variant italic">Nothing to show</span
			>{/if}</pre>
</section>
