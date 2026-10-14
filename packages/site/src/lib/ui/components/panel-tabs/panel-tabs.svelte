<script lang="ts" module>
	export interface PanelTab {
		id: string;
		label: string;
		controls?: string;
	}
</script>

<script lang="ts">
	import { tick } from 'svelte';
	import { cn } from '#lib/ui/utils';

	let {
		id,
		label,
		tabs,
		value,
		onSelect,
		activeClass = 'bg-surface',
		class: className
	}: {
		id: string;
		label: string;
		tabs: readonly PanelTab[];
		value: string;
		onSelect: (id: string) => void;
		activeClass?: string;
		class?: string;
	} = $props();

	let list: HTMLDivElement;

	async function select(tab: PanelTab, focus = false) {
		onSelect(tab.id);

		await tick();

		const button = list.querySelector<HTMLButtonElement>(`[id="${id}-${tab.id}-tab"]`);

		if (focus) button?.focus({ preventScroll: true });

		button?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
	}

	function navigate(event: KeyboardEvent, index: number) {
		let next: number;

		switch (event.key) {
			case 'ArrowRight':
				next = (index + 1) % tabs.length;
				break;
			case 'ArrowLeft':
				next = (index + tabs.length - 1) % tabs.length;
				break;
			case 'Home':
				next = 0;
				break;
			case 'End':
				next = tabs.length - 1;
				break;
			default:
				return;
		}

		event.preventDefault();
		void select(tabs[next], true);
	}
</script>

<div
	bind:this={list}
	role="tablist"
	aria-label={label}
	aria-orientation="horizontal"
	class={cn('flex min-w-0 overflow-x-auto', className)}
>
	{#each tabs as tab, index (tab.id)}
		<button
			type="button"
			role="tab"
			id={`${id}-${tab.id}-tab`}
			aria-controls={tab.controls ?? `${tab.id}-pane`}
			aria-selected={value === tab.id}
			tabindex={value === tab.id ? 0 : -1}
			class={cn(
				'shrink-0 cursor-pointer rounded-none py-1 whitespace-nowrap outline-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-magenta focus-visible:outline-solid',
				value === tab.id
					? cn('font-bold text-on-surface', activeClass)
					: 'bg-transparent text-on-surface-alt hover:bg-surface-bright hover:text-on-surface'
			)}
			onclick={() => select(tab)}
			onkeydown={(event) => navigate(event, index)}
		>
			<span aria-hidden="true" class={cn(value !== tab.id && 'invisible')}>[</span>{tab.label}<span
				aria-hidden="true"
				class={cn(value !== tab.id && 'invisible')}>]</span
			>
		</button>
	{/each}
</div>
