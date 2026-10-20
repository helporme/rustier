import { onDestroy } from 'svelte';
import { isHttpError } from '@sveltejs/kit';
import { build } from './build.remote';
import ExecutionWorker from './execution.worker?worker';
import type { ExecutionMessage } from './execution-protocol';

export class Compilation {
	status = $state<'idle' | 'compiling' | 'running' | 'done' | 'failed'>('idle');
	output = $state('');
	pending = $derived(this.status === 'compiling' || this.status === 'running');
	#worker?: Worker;
	#run = 0;
	#timer?: ReturnType<typeof setTimeout>;

	constructor() {
		onDestroy(this.clear);
	}

	clear = () => {
		this.#run++;
		this.#stop();
		this.status = 'idle';
		this.output = '';
	};

	run = async (code: string, typescript: string) => {
		if (this.pending) return;
		this.clear();
		const run = this.#run;
		this.status = 'compiling';

		try {
			const result = await build({ code, typescript });
			if (run !== this.#run) return;
			if (result.type === 'err') {
				this.#fail(result.message);
				return;
			}

			this.status = 'running';
			const worker = new ExecutionWorker({ name: 'rustier-execution' });
			this.#worker = worker;
			worker.onmessage = ({ data }: MessageEvent<ExecutionMessage>) => {
				if (run !== this.#run || worker !== this.#worker) return;
				if (data.type === 'line') {
					// Bound output from programs that log indefinitely.
					if (this.output.length + data.text.length > 64 * 1024)
						this.#fail('Output limit exceeded');
					else this.output += `${data.text}\n`;
				} else if (data.type === 'error') this.#fail(data.message);
				else {
					this.status = 'done';
					this.#stop();
				}
			};
			worker.onerror = (event) => {
				event.preventDefault();
				if (run === this.#run) this.#fail(event.message || 'Execution failed');
			};
			this.#timer = setTimeout(() => this.#fail('Execution exceeded 10 seconds'), 10_000);
			worker.postMessage(result, [result.wasm.buffer]);
		} catch (cause) {
			if (run === this.#run)
				this.#fail(
					isHttpError(cause)
						? cause.body.message
						: cause instanceof Error
							? cause.message
							: String(cause)
				);
		}
	};

	#stop() {
		clearTimeout(this.#timer);
		this.#worker?.terminate();
		this.#worker = undefined;
	}

	#fail(message: string) {
		this.#stop();
		this.status = 'failed';
		this.output += `${message}\n`;
	}
}
