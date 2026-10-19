import { untrack } from 'svelte';
import InspectorWorker from './inspector.worker?worker';
import type { Inspection, InspectRequest, InspectResponse } from './inspection';

export class Inspector {
	#worker?: Worker;
	#connection = $state.raw<{ worker: Worker } | { error: string }>();
	#request = 0;

	compilation = $state.raw<Inspection>();
	error = $state('');
	pending = $state(false);

	constructor(getSource: () => string | undefined) {
		$effect(() => () => {
			this.#worker?.terminate();
			this.#worker = undefined;
		});

		$effect(() => {
			const code = getSource();
			const connection = this.#connection;
			const id = ++this.#request;

			this.error = '';
			this.pending = code !== undefined;

			if (code === undefined) {
				this.compilation = undefined;
				return;
			}

			if (connection && 'error' in connection) {
				this.compilation = undefined;
				this.error = connection.error;
				this.pending = false;
				return;
			}

			untrack(() => {
				if (!this.#worker) this.#start();
				connection?.worker.postMessage({ id, code } satisfies InspectRequest);
			});
		});
	}

	restart = () => {
		this.#worker?.terminate();
		this.#worker = undefined;
		this.#connection = undefined;
	};

	#start() {
		try {
			const worker = new InspectorWorker({ name: 'rustier-inspector' });

			this.#worker = worker;
			worker.onmessage = ({ data }: MessageEvent<InspectResponse>) => {
				if (worker !== this.#worker) return;

				if (data.type === 'ready') {
					this.#connection = { worker };
					return;
				}

				if (data.type === 'error' && data.fatal) {
					this.#fail(data.message);
					return;
				}

				if (data.id !== this.#request) return;

				this.pending = false;

				if (data.type === 'result') this.compilation = data.compilation;
				else {
					this.compilation = undefined;
					this.error = data.message;
				}
			};
			worker.onerror = (event) => {
				if (worker === this.#worker) this.#fail(event.message || 'Compiler failed to load');
			};
		} catch (cause) {
			this.#fail(String(cause));
		}
	}

	#fail(message: string) {
		this.#worker?.terminate();
		this.#worker = undefined;
		this.compilation = undefined;
		this.pending = false;
		this.error = message;
		this.#connection = { error: message };
	}
}
