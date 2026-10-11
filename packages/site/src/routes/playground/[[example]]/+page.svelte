<script lang="ts">
	import { Button } from '#lib/ui/components/button';
	import { CodeEditor } from '#lib/ui/components/code-editor';
	import { examples } from '#lib/playground/examples';
	import { Compiler } from '../(components)/compiler.svelte.ts';
	import Navigation from '../(components)/navigation.svelte';
	import Panels from '../(components)/panels.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let snippets = $state(examples.map((example) => ({ ...example })));
	const compiler = new Compiler(() => snippets[data.selected].code);

	const representations = [
		{ value: 'ts_code', label: 'TS declarations', language: 'typescript', filename: 'lib.rs.d.ts' },
		{
			value: 'ambient_ts_code',
			label: 'Ambient declarations',
			language: 'typescript',
			filename: 'ambient.d.ts'
		},
		{ value: 'js_code', label: 'JS facade', language: 'typescript', filename: 'lib.rs.js' },
		{ value: 'rust_code', label: 'Rust facade', language: 'rust', filename: 'facade.rs' },
		{ value: 'diagnostics', label: 'Diagnostics', language: 'ansi', filename: 'diagnostics.txt' }
	] as const;
	let representation = $state<(typeof representations)[number]['value']>('ts_code');
	let output = $derived(representations.find((item) => item.value === representation)!);
	let content = $derived(compiler.compilation?.[representation]);
	let status = $derived(compiler.ready ? 'Compiling…' : 'Loading compiler…');
	let value = $derived(
		compiler.error ||
			content ||
			(compiler.pending
				? `# ${status}`
				: representation === 'diagnostics'
					? '# No diagnostics'
					: '# No declarations')
	);
	let language: 'text' | (typeof representations)[number]['language'] = $derived(
		compiler.error || compiler.pending || !content ? 'text' : output.language
	);
</script>

<div class="flex h-dvh min-h-0 flex-col bg-[light-dark(#fff,#000)] text-on-surface">
	<Navigation examples={snippets} selected={snippets[data.selected].slug} />
	<main class="@container min-h-0 min-w-0 flex-1 overflow-hidden" aria-label="playground">
		<Panels>
			{#snippet rust(activate)}
				<CodeEditor
					bind:value={snippets[data.selected].code}
					language="rust"
					filename="lib.rs"
					markers={compiler.markers}
					languageService={compiler}
					onNavigate={activate}
				/>
			{/snippet}
			{#snippet typescript(activate)}
				<CodeEditor
					bind:value={snippets[data.selected].typescript}
					language="typescript"
					filename="index.ts"
					extraLibs={compiler.libraries}
					onNavigate={activate}
					class="bg-inherit"
				/>
			{/snippet}
			{#snippet results(activate)}
				<div class="flex h-full min-h-0 flex-col">
					<div class="flex flex-wrap items-center gap-x-2 px-2 py-1 text-on-surface">
						<select
							aria-label="Generated representation"
							bind:value={representation}
							class="min-w-0 cursor-pointer bg-inherit outline-none"
						>
							{#each representations as item (item.value)}
								<option value={item.value}>{item.label}</option>
							{/each}
						</select>
						{#if compiler.workerError}
							<Button onclick={compiler.activate}>Restart compiler</Button>
						{:else if compiler.pending}
							<span role="status" class="text-on-surface-alt">{status}</span>
						{/if}
					</div>
					<div class="min-h-0 flex-1">
						<CodeEditor
							{value}
							{language}
							filename={output.filename}
							onNavigate={activate}
							readOnly
							colorScheme="rustier-dark-dimmest"
							class="dark bg-surface-dim"
						/>
					</div>
				</div>
			{/snippet}
		</Panels>
	</main>
</div>
