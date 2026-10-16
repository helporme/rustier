<script lang="ts">
	import { onMount } from 'svelte';
	import { Toggle } from '#lib/ui/components/toggle';

	let dark = $state(false);

	onMount(() => {
		const preference = matchMedia('(prefers-color-scheme: dark)');
		const sync = () => {
			const scheme = getComputedStyle(document.documentElement).colorScheme;
			dark = scheme === 'dark' || (scheme !== 'light' && preference.matches);
		};

		sync();
		preference.addEventListener('change', sync);

		return () => preference.removeEventListener('change', sync);
	});

	function setDark(pressed: boolean) {
		dark = pressed;
		document.documentElement.style.colorScheme = pressed ? 'dark' : 'light';
		document.documentElement.classList.toggle('dark', pressed);

		try {
			localStorage.setItem('rustier-theme', pressed ? 'dark' : 'light');
		} catch {
			return;
		}
	}
</script>

<Toggle
	aria-label="Dark theme"
	title={dark ? 'Switch to light theme' : 'Switch to dark theme'}
	pressed={dark}
	onPressedChange={setDark}
	color="cyan"
	class="gap-[1ch]"
>
	<span aria-hidden="true">[~]</span>
	{dark ? 'Dark' : 'Light'}
</Toggle>
