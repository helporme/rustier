import tailwindcss from '@tailwindcss/vite';
import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import rustier from 'rustier/vite';

const browser = { crates: { 'rustier-site': { path: '.' } } };

export default defineConfig({
	envDir: fileURLToPath(new URL('../../', import.meta.url)),
	plugins: [
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
		rustier({ browser })
	],
	ssr: {
		noExternal: [/.*/]
	},
	optimizeDeps: {
		include: ['typescript']
	},
	build: {
		cssTarget: ['chrome123', 'edge123', 'firefox120', 'safari17.5', 'ios17.5']
	},
	worker: {
		plugins: () => [rustier({ browser })],
		format: 'es'
	}
});
