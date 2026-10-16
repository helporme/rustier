<script lang="ts" module>
	import type { load } from './+page.server';

	export type PageData = Awaited<ReturnType<typeof load>>;
	export type PaneSide = 'left' | 'right';
	export type PaneSelection = Record<PaneSide, string>;
	export type PlaygroundMode = 'run' | 'analysis';

	const sources = [
		{ id: 'rust', label: 'lib.rs', language: 'rust', filename: 'lib.rs' },
		{ id: 'typescript', label: 'index.ts', language: 'typescript', filename: 'index.ts' }
	] as const;

	const representations = [
		{ id: 'ts_code', label: 'TS', language: 'typescript', filename: 'lib.rs.d.ts' },
		{
			id: 'ambient_ts_code',
			label: 'TS Ambient',
			language: 'typescript',
			filename: 'ambient.d.ts'
		},
		{ id: 'js_code', label: 'JS Bridge', language: 'typescript', filename: 'lib.rs.js' },
		{ id: 'rust_code', label: 'Rust Bridge', language: 'rust', filename: 'facade.rs' },
		{ id: 'diagnostics', label: 'Diagnostics', language: 'ansi', filename: 'diagnostics.txt' }
	] as const;

	const views = [...sources, ...representations];
</script>

<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { tick } from 'svelte';
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import { CodeEditor, EditorPane } from '#lib/ui/components/code-editor';
	import { Panels } from '#lib/ui/components/panels';
	import { PanelTabs } from '#lib/ui/components/panel-tabs';
	import { PlasmaAnimation } from '#lib/ui/components/plasma-animation';
	import { examples } from '#lib/playground/examples';
	import { Playground } from '#lib/playground/playground.svelte.ts';
	import Navigation from '../(components)/navigation.svelte';

	let { data }: { data: PageData } = $props();

	let snippets = $state(
		examples.map((example) => ({
			...example,
			code: String(example.code),
			typescript: String(example.typescript)
		}))
	);

	const { compiler, builder } = new Playground(
		() => snippets[data.selected],
		() => data.compiled
	);

	let mode: PlaygroundMode = $derived(
		page.url.searchParams.get('mode') === 'analysis' ? 'analysis' : 'run'
	);

	const panelId = $props.id();
	let selectedPanes = $state<{ mode: typeof mode; panes: PaneSelection }>();
	let panes: PaneSelection = $derived(
		selectedPanes?.mode === mode
			? selectedPanes.panes
			: { left: 'rust', right: mode === 'analysis' ? 'ts_code' : 'typescript' }
	);

	let generatedTabs = $derived(
		representations.map((view) =>
			view.id === 'diagnostics'
				? { ...view, label: `Diagnostics ${compiler.compilation?.diagnostic_count ?? 0}` }
				: view
		)
	);

	let tabs = $derived({
		left: mode === 'analysis' ? sources : [sources[0]],
		right:
			mode === 'analysis' ? generatedTabs : [sources[1], generatedTabs[generatedTabs.length - 1]]
	});

	let split = $state(0.5);
	let animateSidebar = $state(false);
	let panelRefs = $state<Partial<Record<PaneSide, HTMLDivElement>>>({});

	function build() {
		animateSidebar = true;
		void builder.build(snippets[data.selected].code, snippets[data.selected].typescript);
	}

	async function open(value: string, side: PaneSide) {
		selectedPanes = { mode, panes: { ...panes, [side]: value } };

		await tick();

		// Focus the updated panel, not Monaco: Enter opts into editing, while Tab skips the editor.
		panelRefs[side]?.focus({ preventScroll: true });
	}

	async function selectMode(mode: PlaygroundMode) {
		const search = new SvelteURLSearchParams(page.url.search);

		if (mode === 'analysis') search.set('mode', 'analysis');
		else search.delete('mode');

		const query = search.toString();
		const route = query
			? (`/playground/[[example]]?${query}${page.url.hash}` as const)
			: page.url.hash
				? (`/playground/[[example]]${page.url.hash}` as `/playground/[[example]]#${string}`)
				: '/playground/[[example]]';

		await goto(resolve(route, { example: page.params.example }), { reset: false });
	}
</script>

<svelte:head>
	<title>{snippets[data.selected].name} · rustier</title>
</svelte:head>

{#snippet output()}
	<PlasmaAnimation busy={builder.status === 'building'}>
		<CodeEditor
			value={builder.output}
			language="ansi"
			filename="output.txt"
			colorScheme="rustier-dark"
			readOnly
			class="[--editor-background:#000]"
		/>
	</PlasmaAnimation>
{/snippet}

{#snippet pane(side: PaneSide)}
	{@const view = views.find((item) => item.id === panes[side])!}
	{@const source = view.id === 'rust' || view.id === 'typescript'}
	{@const content = source ? undefined : compiler.compilation?.[view.id]}
	<div
		class={[
			'flex h-full min-h-0 min-w-0 flex-col bg-(--editor-background)',
			view.id === 'diagnostics'
				? '[--editor-background:var(--color-surface-dimest)]'
				: side === 'left'
					? '[--editor-background:var(--color-surface)]'
					: '[--editor-background:var(--color-surface-dim)]'
		]}
	>
		<PanelTabs
			id={`${panelId}-${side}`}
			label={`${side} pane views`}
			tabs={tabs[side].map((item) => ({ ...item, controls: `${panelId}-${side}-view` }))}
			value={view.id}
			onSelect={(value) => open(value, side)}
			activeClass="bg-inherit"
			class="shrink-0 bg-inherit"
		/>
		<EditorPane
			bind:ref={panelRefs[side]}
			id={`${panelId}-${side}-view`}
			value={view.id}
			labelledby={`${panelId}-${side}-${view.id}-tab`}
		>
			<CodeEditor
				bind:value={
					() =>
						view.id === 'rust'
							? snippets[data.selected].code
							: view.id === 'typescript'
								? snippets[data.selected].typescript
								: compiler.error ||
									content ||
									(compiler.pending
										? ''
										: view.id === 'diagnostics'
											? '# No diagnostics'
											: '# No declarations'),
					(value) => {
						if (view.id === 'rust') snippets[data.selected].code = value;
						else if (view.id === 'typescript') snippets[data.selected].typescript = value;
					}
				}
				language={!source && (compiler.error || !content) ? 'text' : view.language}
				filename={view.filename}
				tabIndex={-1}
				readOnly={!source}
				markers={view.id === 'rust' ? compiler.markers : []}
				initialPreview={view.id === 'rust'
					? data.preview.rust
					: view.id === 'typescript'
						? data.preview.typescript
						: undefined}
				languageService={view.id === 'rust' ? compiler : undefined}
				extraLibs={compiler.libraries}
				onNavigate={() => open(view.id, side)}
				class="bg-inherit"
			/>
		</EditorPane>
	</div>
{/snippet}

<div
	class={[
		'grid h-dvh min-h-0 overflow-hidden text-on-surface',
		animateSidebar &&
			'transition-[grid-template-columns] duration-200 motion-reduce:transition-none',
		builder.status === 'idle' ? 'grid-cols-[minmax(0,1fr)_36ch]' : 'grid-cols-[minmax(0,1fr)_60ch]'
	]}
>
	<main class="min-h-0 min-w-0 overflow-hidden" aria-label="playground">
		<Panels bind:split>
			{#snippet left()}{@render pane('left')}{/snippet}
			{#snippet right()}{@render pane('right')}{/snippet}
		</Panels>
	</main>
	<Navigation
		examples={snippets}
		selected={snippets[data.selected].slug}
		{mode}
		onModeChange={selectMode}
		building={builder.status === 'building'}
		onBuild={build}
		onCloseOutput={builder.clear}
		{output}
		showOutput={builder.status !== 'idle'}
		error={compiler.workerError}
		onRestart={compiler.activate}
	/>
</div>
