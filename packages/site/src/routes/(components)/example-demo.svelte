<script lang="ts" module>
	import { examples } from '../playground/(components)/examples/examples';

	const source = examples[0].typescript;
</script>

<script lang="ts">
	import { Terminal } from '@lucide/svelte';
	import { CodeEditor } from '#lib/ui/components/code-editor';
	import javascriptIcon from '#lib/assets/javascript.svg';
</script>

<figure aria-label="Inline Rust word count and results" class="min-w-0 lg:pt-8">
	<div class="grid min-w-0 gap-3">
		<div
			data-code-file="index.js"
			class="flex h-80 min-w-0 flex-col overflow-hidden rounded-lg border border-ink-outline bg-ink-surface-bright shadow-sm [view-transition-name:playground-source]"
		>
			<div
				class="flex h-9 shrink-0 items-stretch bg-ink-surface after:flex-1 after:border-b after:border-ink-outline"
			>
				<div
					class="flex items-center gap-2 border-r border-ink-outline bg-ink-surface-bright px-4 font-code text-ui"
				>
					<span
						aria-hidden="true"
						class="size-4 shrink-0 bg-current"
						style:mask={`url(${JSON.stringify(javascriptIcon)}) center / contain no-repeat`}
					></span>
					index.js
				</div>
			</div>
			<div class="min-h-0 flex-1">
				<CodeEditor
					value={source}
					language="typescript"
					label="JavaScript source preview"
					readonly
				/>
			</div>
		</div>
		<div
			class="overflow-hidden rounded-lg border border-ink-outline bg-ink-surface-bright shadow-sm [view-transition-name:playground-output]"
		>
			<div
				class="flex h-9 items-stretch bg-ink-surface after:flex-1 after:border-b after:border-ink-outline"
			>
				<div
					class="flex items-center gap-2 border-r border-ink-outline bg-ink-surface-bright px-4 font-code text-ui"
				>
					<Terminal aria-hidden="true" class="size-4 shrink-0" />
					Results
				</div>
			</div>
			<pre class="overflow-auto px-6 pt-3 pb-4 font-mono text-code whitespace-pre-wrap">4</pre>
		</div>
	</div>
</figure>
