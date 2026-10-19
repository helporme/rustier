<script lang="ts" module>
	import { ScanSearch } from '@lucide/svelte';
	import { Toggle, type ToggleVariant } from '#lib/ui/components/toggle';

	const variants = [
		'primary',
		'alternative',
		'outline',
		'ghost'
	] as const satisfies readonly ToggleVariant[];
</script>

<script lang="ts">
	let pressed = $state({ primary: false, alternative: false, outline: false, ghost: false });
</script>

<section aria-labelledby="toggles" class="space-y-4">
	<h2 id="toggles" class="font-brand text-2xl">Toggle</h2>
	<p class="text-ui text-on-ink-surface-variant">
		Use click, Space, or Enter to toggle. Primary is the default; alternative uses the secondary
		theme.
	</p>
	{#each variants as variant (variant)}
		<div class="flex flex-wrap items-center gap-4">
			<span class="w-20 font-code text-ui">{variant}</span>
			<Toggle {variant} bind:pressed={pressed[variant]} aria-label={`${variant} toggle`}>
				<ScanSearch aria-hidden="true" class="size-4" /> Toggle
			</Toggle>
			<Toggle {variant} pressed aria-label={`${variant} initially pressed`}>Pressed</Toggle>
			<Toggle {variant} disabled aria-label={`${variant} disabled`}>Disabled</Toggle>
			<Toggle {variant} disabled pressed aria-label={`${variant} disabled pressed`}
				>Disabled on</Toggle
			>
		</div>
	{/each}
	<div class="flex flex-wrap items-center gap-4">
		<Toggle size="lg">Large toggle</Toggle>
		<Toggle variant="alternative" size="icon" aria-label="Icon-only toggle">
			<ScanSearch aria-hidden="true" class="size-4" />
		</Toggle>
	</div>
	<p role="status" class="text-ui">
		Pressed: {variants.filter((variant) => pressed[variant]).join(', ') || 'none'}
	</p>
</section>
