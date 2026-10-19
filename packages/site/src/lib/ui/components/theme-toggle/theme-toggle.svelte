<script lang="ts">
	import { Moon, Sun } from '@lucide/svelte';
	import { toggleMode } from 'mode-watcher';
	import { Button } from '#lib/ui/components/button';
</script>

<Button
	variant="outline"
	size="icon"
	class="size-10"
	aria-label="Toggle color theme"
	title="Toggle light and dark theme"
	onclick={toggleMode}
>
	<Sun aria-hidden="true" class="size-4 [.dark_&]:hidden" />
	<Moon aria-hidden="true" class="hidden size-4 [.dark_&]:block" />
</Button>
