<script lang="ts" module>
	import { Button, type ButtonVariant } from '#lib/ui/components/button';

	const variants: ButtonVariant[] = ['default', 'outline', 'ghost'];
</script>

<script lang="ts">
	let clicks = $state(0);
</script>

<section aria-labelledby="buttons" class="space-y-4">
	<h2 id="buttons" class="font-brand text-2xl">Buttons</h2>
	<p class="text-ui text-on-ink-surface-variant">
		Hover, press, or use Tab and Enter to inspect each state.
	</p>
	{#each variants as variant (variant)}
		<div class="flex flex-wrap items-center gap-4">
			<span class="w-20 font-code text-ui">{variant}</span>
			<Button {variant} onclick={() => clicks++}>Try me</Button>
			<Button {variant} disabled>Disabled</Button>
			<Button {variant} loading>Working…</Button>
		</div>
	{/each}
	<div class="flex flex-wrap items-center gap-4">
		<Button size="lg" onclick={() => clicks++}>Large action ↗</Button>
		<Button size="icon" aria-label="Increment button clicks" onclick={() => clicks++}>+</Button>
	</div>
	<p role="status" class="text-ui">Button clicks: {clicks}</p>
</section>
