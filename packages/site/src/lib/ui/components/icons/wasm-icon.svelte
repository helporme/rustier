<script lang="ts">
	import type { SVGAttributes } from 'svelte/elements';
	import Icon from './icon.svelte';

	let props: SVGAttributes<SVGSVGElement> = $props();
</script>

<Icon viewBox="0 0 24 24" data-icon="wasm" {...props}>
	<!-- WebAssembly mark from Simple Icons. -->
	<path
		d="M14.745 0v.129a2.752 2.752 0 1 1-5.504 0V0H0v24h24V0zM11.454 21.431l-1.169-5.783h-.02l-1.264 5.783H7.39l-1.824-8.497h1.59l1.088 5.783h.02l1.311-5.783h1.487l1.177 5.854h.02l1.242-5.854h1.561l-2.027 8.497zm8.755 0-.542-1.891h-2.861l-.417 1.891h-1.59l2.056-8.497h2.509l2.5 8.497zm-2.397-6.403-.694 3.118h2.159l-.796-3.118z"
		class="fill-current"
	/>
</Icon>
