<script lang="ts" module>
	const files = [
		{
			name: 'greet.rs',
			language: 'rust',
			label: 'Rust',
			source: 'pub fn greet(name: &str) -> String {\n    format!("Hello {name}!")\n}'
		},
		{
			name: 'main.ts',
			language: 'typescript',
			label: 'TypeScript',
			source: "import { greet } from './greet.rs'\n\nconsole.log(greet('World'))"
		}
	] as const;
</script>

<script lang="ts">
	import { CodeEditor } from '#lib/ui/components/code-editor';
	import rustIcon from '#lib/assets/rust.svg';
	import typescriptIcon from '#lib/assets/typescript.svg';
</script>

<figure aria-label="Importing a Rust greeting in TypeScript" class="min-w-0 lg:pt-8">
	<div class="grid gap-3">
		{#each files as file (file.name)}
			<div
				data-code-file={file.name}
				style:view-transition-name={file.language === 'rust' ? 'rust-editor' : 'typescript-editor'}
				class="flex h-44 min-w-0 flex-col overflow-hidden rounded-lg border border-ink-outline bg-ink-surface-bright shadow-sm"
			>
				<div
					class="flex h-9 shrink-0 items-stretch bg-ink-surface after:flex-1 after:border-b after:border-ink-outline"
				>
					<div
						class="flex min-w-30 items-center gap-2 border-r border-ink-outline bg-ink-surface-bright px-4 font-code text-ui"
					>
						{#if file.language === 'rust'}
							<span
								aria-hidden="true"
								class="size-4 shrink-0 bg-current"
								style:mask={`url(${rustIcon}) center / contain no-repeat`}
							></span>
						{:else}
							<img src={typescriptIcon} alt="" class="size-4 shrink-0" />
						{/if}
						{file.name}
					</div>
				</div>
				<div class="min-h-0 flex-1">
					<CodeEditor
						value={file.source}
						language={file.language}
						label={`${file.label} source preview`}
						readonly
					/>
				</div>
			</div>
		{/each}
	</div>
	<figcaption
		class="mt-4 rounded-md border border-ink-outline bg-ink-surface px-4 py-2 font-code text-ui"
	>
		<span class="mr-4 text-on-ink-surface-variant">Output</span>Hello World!
	</figcaption>
</figure>
