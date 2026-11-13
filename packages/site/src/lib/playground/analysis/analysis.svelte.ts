import { untrack } from 'svelte';
import AnalysisWorker from './analysis.worker?worker';
import type { Analysis, AnalysisRequest, AnalysisResponse } from './protocol';

export class Analyzer {
	output = $state.raw<Analysis>();
	error = $state('');
	pending = $state(true);
	#worker?: Worker;
	#ready = false;
	#busy = false;
	#request = 0;
	#latest?: AnalysisRequest;
	#timer?: ReturnType<typeof setTimeout>;

	constructor(getSource: () => string) {
		$effect(() => {
			const code = getSource();
			untrack(() => {
				this.#latest = { id: ++this.#request, code };
				this.output = undefined;
				if (this.error) return;
				this.pending = true;
				if (!this.#worker) this.#start();
				this.#send();
			});
		});
		$effect(() => () => this.#stop());
	}

	restart = () => {
		this.#stop();
		this.error = '';
		this.output = undefined;
		this.pending = true;
		this.#start();
	};

	#start() {
		try {
			const worker = new AnalysisWorker({ name: 'rustier-analysis' });
			this.#worker = worker;
			this.#timer = setTimeout(
				() => this.#fail('Browser analysis timed out. Restart to try again.'),
				60_000
			);
			worker.onmessage = ({ data }: MessageEvent<AnalysisResponse>) => {
				if (worker !== this.#worker) return;
				clearTimeout(this.#timer);
				if (data.type === 'ready') {
					this.#ready = true;
					this.#send();
					return;
				}
				this.#busy = false;
				if (data.type === 'error' && data.fatal) {
					this.#fail(data.message);
					return;
				}
				if (data.id !== this.#latest?.id) {
					this.#send();
					return;
				}
				this.pending = false;
				if (data.type === 'result') {
					this.output = data.analysis;
					this.error = '';
				} else {
					this.#fail(data.message);
				}
			};
			worker.onerror = (event) => {
				event.preventDefault();
				if (worker === this.#worker) this.#fail(event.message || 'Browser compiler failed to load');
			};
		} catch (cause) {
			this.#fail(String(cause));
		}
	}

	#send() {
		if (!this.#ready || this.#busy || !this.#latest) return;
		this.#busy = true;
		this.#timer = setTimeout(
			() => this.#fail('Browser analysis timed out. Restart to try again.'),
			60_000
		);
		this.#worker!.postMessage(this.#latest);
	}

	#stop() {
		clearTimeout(this.#timer);
		this.#worker?.terminate();
		this.#worker = undefined;
		this.#ready = false;
		this.#busy = false;
	}

	#fail(message: string) {
		this.#stop();
		this.output = undefined;
		this.pending = false;
		this.error = message;
	}
}
