<script lang="ts" module>
	import { Monitor, Moon, Sun } from '@lucide/svelte';

	const options = [
		{ value: 'light', label: 'Light', icon: Sun },
		{ value: 'dark', label: 'Dark', icon: Moon },
		{ value: 'system', label: 'System', icon: Monitor }
	] as const;
</script>

<script lang="ts">
	import { setMode, systemPrefersMode, userPrefersMode } from 'mode-watcher';
	import { Button } from '#lib/ui/components/button';

	const current = $derived(
		options.find((option) => option.value === userPrefersMode.current) ?? options[2]
	);
	const cycle = $derived(
		systemPrefersMode.current === 'dark'
			? [options[2], options[0], options[1]]
			: [options[2], options[1], options[0]]
	);
	const next = $derived(cycle[(cycle.indexOf(current) + 1) % cycle.length]);
</script>

<Button
	variant="ghost"
	size="icon"
	class="size-10 border-0"
	aria-label={`Color theme: ${current.label}. Switch to ${next.label}`}
	title={`Color theme: ${current.label}. Switch to ${next.label}`}
	onclick={() => setMode(next.value)}
>
	<current.icon aria-hidden="true" class="size-4" />
</Button>
