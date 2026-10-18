<script lang="ts">
	import type { SVGAttributes } from 'svelte/elements';

	let props: SVGAttributes<SVGSVGElement> = $props();
</script>

<svg
	xmlns="http://www.w3.org/2000/svg"
	role="img"
	aria-label="Rustier"
	width="160"
	height="160"
	{...props}
	viewBox="0 0 160 160"
>
	<path
		d="M35 23Q78 16 126 25Q139 27 137 44L133 126Q132 140 116 139L35 135Q20 134 22 118L24 40Q24 25 35 23Z"
		fill="var(--color-java, #f5d75d)"
		stroke="var(--color-ink, #24201f)"
		stroke-width="6"
		stroke-linejoin="round"
	/>
	<path d="M105 23q22 0 30 7l1 34q-19-1-20-16-18 4-20-9Z" fill="var(--color-rust, #cf7752)" />
	<g fill="var(--color-ink, #24201f)">
		<ellipse cx="56" cy="58" rx="5" ry="7" />
		<ellipse cx="91" cy="56" rx="5" ry="7" />
	</g>
	<g fill="none" stroke="var(--color-ink, #24201f)" stroke-width="7" stroke-linecap="round">
		<path d="M71 87v25q0 11-14 11-9 0-13-8M116 91q-19-10-22 3-2 7 11 11t9 13q-7 10-22 0" />
	</g>
	<circle cx="119" cy="37" r="4" fill="var(--color-java, #f5d75d)" />
</svg>
