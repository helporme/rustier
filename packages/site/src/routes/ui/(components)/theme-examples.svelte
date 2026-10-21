<script lang="ts" module>
	import { Logo } from '#lib/ui/components/icons';

	const tokens = [
		'primary',
		'on-primary',
		'on-primary-variant',
		'primary-surface',
		'primary-surface-dim',
		'on-primary-surface',
		'on-primary-surface-dim',
		'on-primary-surface-variant',
		'primary-outline',
		'secondary',
		'on-secondary',
		'on-secondary-variant',
		'secondary-surface',
		'secondary-surface-dim',
		'on-secondary-surface',
		'on-secondary-surface-dim',
		'on-secondary-surface-variant',
		'secondary-outline',
		'java',
		'on-java',
		'on-java-variant',
		'java-surface',
		'java-surface-dim',
		'on-java-surface',
		'on-java-surface-dim',
		'on-java-surface-variant',
		'java-outline',
		'rust',
		'on-rust',
		'on-rust-variant',
		'rust-surface',
		'rust-surface-dim',
		'on-rust-surface',
		'on-rust-surface-dim',
		'on-rust-surface-variant',
		'rust-outline',
		'ink',
		'on-ink',
		'on-ink-variant',
		'ink-surface',
		'ink-surface-bright',
		'on-ink-surface',
		'on-ink-surface-variant',
		'ink-outline',
		'editor-keyword',
		'editor-type',
		'editor-string',
		'editor-number',
		'editor-function',
		'editor-comment',
		'editor-warning',
		'editor-error',
		'editor-selection',
		'editor-active-line'
	];
</script>

<section aria-labelledby="theme" class="space-y-4">
	<h2 id="theme" class="font-brand text-2xl">Theme</h2>
	<div class="flex flex-wrap items-center gap-8">
		<Logo class="size-24" />
		<p class="font-brand text-wordmark">Lilita One — Rustier</p>
		<p class="font-body">Nunito Sans — interface text</p>
		<p class="font-code text-code">Iosevka — let answer = 42;</p>
	</div>
	<div class="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-5">
		{#each tokens as token (token)}
			<div class="min-w-0">
				{#if token.startsWith('on-') || ['editor-keyword', 'editor-type', 'editor-string', 'editor-number', 'editor-function', 'editor-comment', 'editor-warning', 'editor-error'].includes(token)}
					<div
						class="grid h-14 place-items-center rounded-lg border border-ink-outline bg-ink-surface font-code text-code"
						style:color={`var(--color-${token})`}
					>
						Aa
					</div>
				{:else}
					<div
						class="h-14 rounded-lg border border-ink-outline"
						style:background-color={`var(--color-${token})`}
					></div>
				{/if}
				<p class="mt-2 font-code text-ui break-words">{token}</p>
			</div>
		{/each}
	</div>
</section>
