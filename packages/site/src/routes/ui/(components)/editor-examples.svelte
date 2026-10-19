<script lang="ts">
	import { Button } from '#lib/ui/components/button';
	import { CodeEditor, type CodeLanguage } from '#lib/ui/components/code-editor';
	import * as Select from '#lib/ui/components/select';

	let value = $state('pub fn greet() -> String {\n    "Hello, friend!".to_owned()\n}');
	let language = $state<CodeLanguage>('rust');
	let readonly = $state(false);
	let disabled = $state(false);
</script>

<section aria-labelledby="editor" class="space-y-4">
	<h2 id="editor" class="font-brand text-2xl">Code editor</h2>
	<p class="text-ui text-on-ink-surface-variant">
		Syntax highlighting, completion, bracket matching, undo/redo, and search. Press Escape then Tab
		to leave the editor.
	</p>
	<div class="flex flex-wrap items-center gap-4">
		<Select.Root type="single" bind:value={language} allowDeselect={false}>
			<Select.Trigger aria-label="Editor language">{language}</Select.Trigger>
			<Select.Content>
				<Select.Item value="rust" label="Rust" />
				<Select.Item value="typescript" label="TypeScript" />
				<Select.Item value="text" label="Plain text" />
			</Select.Content>
		</Select.Root>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={readonly} /> Read only</label
		>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={disabled} /> Disabled</label
		>
		<Button variant="outline" onclick={() => (value = '')}>Clear editor</Button>
		<Button variant="outline" onclick={() => (value = 'const answer = 42;')}>Replace value</Button>
	</div>
	<div class="h-64 overflow-hidden rounded-lg border border-ink-outline">
		<CodeEditor
			bind:value
			{language}
			{readonly}
			{disabled}
			label="Demo editor"
			placeholder="Write some code…"
		/>
	</div>
	<p class="text-ui text-on-ink-surface-variant">Current value ({value.length} characters)</p>
	<pre class="overflow-auto rounded-lg bg-ink-surface p-4 font-code text-code">{value}</pre>
</section>
