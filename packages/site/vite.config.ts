import tailwindcss from '@tailwindcss/vite';
import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import rustier from 'rustier/vite';
import { compilerService } from './dev/compiler.ts';

export default defineConfig(({ command, isPreview }) => ({
	plugins: [
		command === 'serve' && !isPreview && compilerService(),
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true,
				experimental: { async: true }
			},

			adapter: adapter(),
			experimental: { remoteFunctions: true }
		}),
		rustier()
	],
	ssr: {
		noExternal: [/.*/],
		external: ['rustier', 'rustier-native']
	},
	build: {
		cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari17.5', 'ios17.5']
	},
	worker: {
		plugins: () => [rustier()],
		format: 'es'
	}
}));
