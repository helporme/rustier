import { build } from './build.remote';
import { execute, type ExecutionLevel } from './execution';
import type { BuildResult, ExampleCompilation } from './compilation-schema';
import { SvelteMap } from 'svelte/reactivity';

const CACHED_RUN_DELAY_MS = 300;
const colors = { log: '', info: '', debug: '\x1b[2m', warn: '\x1b[33m', error: '\x1b[31m' };
export type BuildStatus = 'idle' | 'building' | 'running' | 'done' | 'failed';

export class Builder {
	status = $state<BuildStatus>('idle');
	summary = $state('');
	failure = $state('');
	entries = $state<{ level: ExecutionLevel; text: string }[]>([]);
	output = $derived(
		this.status === 'idle'
			? ''
			: [
					...(this.summary ? [`# ${this.summary}`] : []),
					...(this.failure ? [this.failure] : []),
					...this.entries.map(({ level, text }) =>
						colors[level] ? `${colors[level]}${text}\x1b[0m` : text
					)
				].join('\n')
	);
	#urls: string[] = [];
	#run = 0;
	#examples = new SvelteMap<string, ExampleCompilation['result']>();
	#latest?: { code: string; result: Promise<BuildResult> };

	seed(example: ExampleCompilation) {
		this.#examples.set(example.code, example.result);
	}

	clear = () => {
		this.#run++;
		this.status = 'idle';
		this.summary = '';
		this.failure = '';
		this.entries = [];
	};

	build = async (rust: string, typescript: string) => {
		this.clear();

		const run = this.#run;

		this.status = 'building';
		this.summary = 'Preparing run';
		this.#urls.forEach((url) => URL.revokeObjectURL(url));
		this.#urls = [];

		try {
			const { transpile } = await import('../ui/components/code-editor/monaco');

			if (run !== this.#run) return;

			const code = await transpile(typescript, 'index.ts');

			if (run !== this.#run) return;

			const example = this.#examples.get(rust);
			const cached = example
				? Promise.resolve(example)
				: this.#latest?.code === rust
					? this.#latest.result
					: undefined;
			const compilation = cached ?? build({ code: rust });

			if (!cached) {
				this.summary = 'Compiling lib.rs';
				this.#latest = { code: rust, result: compilation };
			}

			let result: BuildResult;

			try {
				const delay = cached
					? new Promise<void>((resolve) => setTimeout(resolve, CACHED_RUN_DELAY_MS))
					: undefined;

				[result] = await Promise.all([compilation, delay]);
			} catch (cause) {
				if (this.#latest?.result === compilation) this.#latest = undefined;

				throw cause;
			}

			if (run !== this.#run) return;

			if (result.type === 'err') {
				this.status = 'failed';
				this.summary = 'Compilation failed';
				this.failure = result.message;
				return;
			}

			const url = (part: BlobPart) => {
				const created = URL.createObjectURL(
					new Blob([part], {
						type: part instanceof Uint8Array ? 'application/wasm' : 'text/javascript'
					})
				);

				this.#urls.push(created);
				return created;
			};
			const binary = url(result.wasm as Uint8Array<ArrayBuffer>);
			const bindings = url(result.glue);
			const ffi = url(
				`import init from ${JSON.stringify(bindings)};\nawait init({ module_or_path: ${JSON.stringify(binary)} });\nexport * from ${JSON.stringify(bindings)};\n`
			);
			const bundle = url(
				result.bundle.replaceAll(JSON.stringify(result.ffiModuleId), JSON.stringify(ffi))
			);

			this.status = 'running';
			this.summary = '';

			const namespaces: Record<string, object> = await import(/* @vite-ignore */ bundle);

			if (run !== this.#run) return;

			const wrappers = Object.fromEntries(
				Object.entries(result.exports).map(([specifier, name]) => {
					const names = Object.keys(namespaces[name]);
					const bindings = names
						.map((key, index) => `const e${index} = ${name}[${JSON.stringify(key)}];`)
						.join('\n');
					const exports = names
						.map((key, index) => `e${index} as ${JSON.stringify(key)}`)
						.join(', ');

					return [
						specifier,
						url(
							`import { ${name} } from ${JSON.stringify(bundle)};\n${bindings}\nexport { ${exports} };\n`
						)
					];
				})
			);

			await execute(code, wrappers, (entry) => {
				if (run === this.#run) this.entries.push(entry);
			});

			if (run === this.#run) this.status = 'done';
		} catch (error) {
			if (run !== this.#run) return;

			this.status = 'failed';
			this.summary = 'Run failed';
			this.failure = error instanceof Error ? error.message : String(error);
		}
	};
}
