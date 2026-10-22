<script lang="ts">
	import { Copy, Bookmark } from '@lucide/svelte';
	import { Button } from '#lib/ui/components/button';
	import { Input } from '#lib/ui/components/input';
	import * as Popover from '#lib/ui/components/popover';
	import { MAX_TITLE_LENGTH } from './share';

	let {
		title,
		saving,
		copied,
		error,
		onsave
	}: {
		title: string;
		saving: boolean;
		copied: boolean;
		error: string;
		onsave: (title: string, copy: boolean) => Promise<boolean>;
	} = $props();

	const id = $props.id();
	let open = $state(false);
	let draftTitle = $state('');

	async function save(event: SubmitEvent) {
		event.preventDefault();
		const copy = event.submitter instanceof HTMLButtonElement && event.submitter.value === 'copy';
		if (await onsave(draftTitle, copy)) open = false;
	}
</script>

<Popover.Root
	bind:open
	onOpenChange={(next) => {
		if (next) draftTitle = title;
	}}
>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button
				{...props}
				variant="ghost"
				size="icon"
				class="size-10"
				aria-label={copied ? 'Link copied' : 'Save snippet'}
				title={copied ? 'Link copied' : 'Save snippet'}
			>
				{#if copied}
					<Copy aria-hidden="true" class="size-4" />
				{:else}
					<Bookmark aria-hidden="true" class="size-4" />
				{/if}
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content align="end" aria-labelledby={`${id}-heading`}>
		<form onsubmit={save} class="space-y-4">
			<h2 id={`${id}-heading`} class="font-bold">Save snippet</h2>
			<div class="space-y-2">
				<label for={`${id}-title`}>Title</label>
				<Input
					id={`${id}-title`}
					bind:value={draftTitle}
					maxlength={MAX_TITLE_LENGTH}
					required
					placeholder="My Rust snippet"
					aria-describedby={error ? `${id}-error` : undefined}
				/>
			</div>
			{#if error}
				<p id={`${id}-error`} role="alert">{error}</p>
			{/if}
			<div class="flex grow flex-wrap gap-3">
				<Button type="submit" disabled={saving || !draftTitle.trim()} aria-busy={saving}>
					<Bookmark aria-hidden="true" class="size-4" />
					Save
				</Button>
				<Button
					type="submit"
					value="copy"
					variant="outline"
					disabled={saving || !draftTitle.trim()}
					aria-busy={saving}
					class="grow"
				>
					<Copy aria-hidden="true" class="size-4" />
					Copy link
				</Button>
			</div>
		</form>
	</Popover.Content>
</Popover.Root>
