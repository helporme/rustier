<script lang="ts">
	import { Microscope, Play } from '@lucide/svelte';
	import { Logo } from '#lib/ui/components/icons';
	import { Button } from '#lib/ui/components/button';
	import { Toggle } from '#lib/ui/components/toggle';
	import { resolve } from '$app/paths';
	import ExamplePicker from './examples/example-picker.svelte';
	import SavePopover from './snippets/save-popover.svelte';
	import type { SavedSnippets } from './snippets/saved-snippets.svelte';

	let {
		selected,
		title,
		saved,
		inspect = $bindable(false),
		pending = false,
		saving = false,
		copied = false,
		saveError = '',
		onsave,
		onrun
	}: {
		selected: string;
		title: string;
		saved: SavedSnippets;
		inspect: boolean;
		pending?: boolean;
		saving?: boolean;
		copied?: boolean;
		saveError?: string;
		onsave: (title: string, copy: boolean) => Promise<boolean>;
		onrun: () => void;
	} = $props();
</script>

<header
	class="flex min-h-15 shrink-0 items-center justify-between gap-2 border-b border-ink-outline bg-rust-surface px-3 py-2 text-on-ink-surface sm:gap-4 sm:px-6"
>
	<div class="flex min-w-0 flex-1 items-center gap-2 sm:gap-4">
		<h1 class="flex shrink-0 items-center gap-3">
			<a href={resolve('/')} aria-label="Rustier home" class="flex items-center gap-2">
				<Logo class="size-10 [view-transition-name:rustier-logo]" aria-hidden="true" />
				<span
					class="hidden font-brand text-wordmark [view-transition-name:rustier-wordmark] sm:inline"
					>rustier</span
				>
			</a>
			<span class="sr-only">playground</span>
		</h1>
		<ExamplePicker {selected} {title} {saved} />
	</div>
	<div class="flex shrink-0 items-center gap-2">
		<SavePopover {title} {saving} {copied} error={saveError} {onsave} />
		<Toggle
			variant="alternative"
			bind:pressed={inspect}
			aria-label="Inspect"
			class="w-10 px-0 min-[30rem]:w-auto min-[30rem]:px-3 sm:px-5"
		>
			<Microscope aria-hidden="true" class="size-4" />
			<span class="hidden min-[30rem]:inline">Inspect</span>
		</Toggle>
		<Button
			onclick={onrun}
			disabled={pending}
			aria-busy={pending}
			aria-label={pending ? 'Running…' : 'Run'}
			class="w-10 px-0 min-[30rem]:w-auto min-[30rem]:px-5"
		>
			<Play aria-hidden="true" class="size-3 fill-current" />
			<span class="hidden min-[30rem]:inline">{pending ? 'Running…' : 'Run'}</span>
		</Button>
	</div>
</header>
