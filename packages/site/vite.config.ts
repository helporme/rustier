import tailwindcss from '@tailwindcss/vite';
import adapter from '@sveltejs/adapter-auto';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import rustier from 'rustier/vite';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true,
				experimental: { async: true }
			},

			// adapter-auto only supports some environments, see https://svelte.dev/docs/kit/adapter-auto for a list.
			// If your environment is not supported, or you settled on a specific environment, switch out the adapter.
			// See https://svelte.dev/docs/kit/adapters for more information about adapters.
			adapter: adapter(),
			experimental: { remoteFunctions: true }
		}),
		rustier()
	],
	ssr: {
		external: ['rustier', 'rustier-native']
	},
	build: {
		cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari17.5', 'ios17.5']
	},
	worker: {
		plugins: () => [rustier()],
		format: 'es'
	}
});
