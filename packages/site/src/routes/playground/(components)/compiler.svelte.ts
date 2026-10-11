import { untrack } from 'svelte';
import { SvelteMap } from 'svelte/reactivity';
import CompilerWorker from '#lib/playground/compiler.worker?worker';
import type {
	Compilation,
	CompileRequest,
	CompilerMessage,
	LanguageRequest,
	LanguageResponse
} from '#lib/playground/compilation';
import type { LanguageService } from '#lib/ui/components/code-editor';

export class Compiler implements LanguageService {
	#compilation = $state.raw<Compilation>();
	#error = $state('');
	#worker = $state.raw<Worker>();
	#activeWorker: Worker | undefined;
	#request = 0;
	#queryId = 0;
	#queries = new SvelteMap<number, (response?: LanguageResponse) => void>();

	pending = $state(true);
	workerError = $state('');
	ready = $derived(!!this.#worker);
	markers = $derived(this.#compilation?.rust_markers ?? []);
	libraries = $derived.by(() => {
		const compilation = this.#compilation;

		if (!compilation?.ts_code || !compilation.ts_sourcemap) return [];

		return [
			{ filePath: 'file:///playground/lib.rs.d.ts', content: compilation.ts_code },
			{ filePath: 'file:///playground/lib.rs.d.ts.map', content: compilation.ts_sourcemap },
			{ filePath: 'file:///playground/ambient.d.ts', content: compilation.ambient_ts_code }
		];
	});

	get compilation() {
		return this.#compilation;
	}

	get error() {
		return this.#error;
	}

	constructor(getSource: () => string) {
		$effect(() => {
			untrack(() => this.activate());

			return () => untrack(() => this.#stop());
		});

		$effect(() => {
			const worker = this.#worker;
			const code = getSource();
			const failure = this.workerError;
			const id = ++this.#request;

			this.#compilation = undefined;
			this.#error = failure;
			this.pending = !failure;

			if (!worker || failure) return;

			const timeout = setTimeout(() => {
				worker.postMessage({ id, code } satisfies CompileRequest);
			}, 150);

			return () => clearTimeout(timeout);
		});
	}

	activate = () => {
		this.#stop();
		this.workerError = '';

		let worker: Worker;

		try {
			worker = new CompilerWorker({ name: 'rustier-compiler' });
		} catch (cause) {
			this.#fail(String(cause));
			return;
		}

		this.#activeWorker = worker;
		worker.onmessage = ({ data }: MessageEvent<CompilerMessage>) => {
			if (worker !== this.#activeWorker) return;

			if ('type' in data) {
				if (data.type === 'ready') {
					this.#worker = worker;
				} else {
					this.#queries.get(data.id)?.(data);
					this.#queries.delete(data.id);
				}

				return;
			}

			if ('error' in data && data.fatal) {
				this.#fail(`Compiler crashed: ${data.error}`);
				return;
			}

			if ('error' in data && this.#queries.has(data.id)) {
				this.#queries.get(data.id)?.();
				this.#queries.delete(data.id);
				return;
			}

			if (data.id !== this.#request) return;

			this.pending = false;

			if ('compilation' in data) {
				this.#compilation = data.compilation;
				this.#error = '';
			} else {
				this.#compilation = undefined;
				this.#error = data.error;
			}
		};
		worker.onerror = (event) => {
			if (worker !== this.#activeWorker) return;

			this.#fail(event.message || 'Compiler failed to load');
		};
	};

	async hover(code: string, offset: number, signal: AbortSignal) {
		const response = await this.#query('hover', code, offset, signal);

		return response?.type === 'hover' ? response.hover : undefined;
	}

	async definition(code: string, offset: number, signal: AbortSignal) {
		const response = await this.#query('definition', code, offset, signal);

		return response?.type === 'definition' ? response.definition : undefined;
	}

	#query(
		type: LanguageRequest['type'],
		code: string,
		offset: number,
		signal: AbortSignal
	): Promise<LanguageResponse | undefined> {
		const worker = this.#worker;

		if (!worker || signal.aborted) return Promise.resolve(undefined);

		const id = --this.#queryId;

		return new Promise((resolve) => {
			const abort = () => {
				this.#queries.delete(id);
				resolve(undefined);
			};

			this.#queries.set(id, (response) => {
				signal.removeEventListener('abort', abort);
				resolve(response);
			});
			signal.addEventListener('abort', abort, { once: true });
			worker.postMessage({ type, id, code, offset } satisfies LanguageRequest);
		});
	}

	#fail(message: string) {
		this.#stop();
		this.workerError = message;
	}

	#stop() {
		this.#activeWorker?.terminate();
		this.#activeWorker = undefined;
		this.#worker = undefined;

		for (const resolve of this.#queries.values()) resolve();

		this.#queries.clear();
	}
}
