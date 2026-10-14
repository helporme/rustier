import { build } from './build.remote';
import { execute, type ExecutionLevel } from './execution';

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
					`# ${this.summary}`,
					...(this.failure ? [this.failure] : []),
					...this.entries.map(({ level, text }) =>
						colors[level] ? `${colors[level]}${text}\x1b[0m` : text
					)
				].join('\n')
	);
	#urls: string[] = [];
	#run = 0;

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
		this.summary = 'Building lib.rs';
		this.#urls.forEach((url) => URL.revokeObjectURL(url));
		this.#urls = [];

		try {
			const { transpile } = await import('../ui/components/code-editor/monaco');
			const code = await transpile(typescript, 'index.ts');
			const modules = [...code.matchAll(/(['"])(\$rust\/[^'"]+)\1/g)].map((match) => match[2]);
			const result = await build({ code: rust, modules });
			const duration = `${(result.duration / 1000).toFixed(2)} s`;

			if (run !== this.#run) return;

			if (result.type === 'err') {
				this.status = 'failed';
				this.summary = `Build failed in ${duration}`;
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
			this.summary = `Built in ${duration}`;

			const namespaces: Record<string, object> = await import(/* @vite-ignore */ bundle);
			const wrappers = Object.fromEntries(
				Object.entries(result.exports).map(([specifier, name]) => [
					specifier,
					url(
						`import { ${name} } from ${JSON.stringify(bundle)};\nexport const { ${Object.keys(namespaces[name]).join(', ')} } = ${name};\n`
					)
				])
			);

			await execute(code, wrappers, (entry) => {
				if (run === this.#run) this.entries.push(entry);
			});

			if (run === this.#run) this.status = 'done';
		} catch (error) {
			if (run !== this.#run) return;

			this.status = 'failed';
			this.summary = 'Build failed';
			this.failure = error instanceof Error ? error.message : String(error);
		}
	};
}
