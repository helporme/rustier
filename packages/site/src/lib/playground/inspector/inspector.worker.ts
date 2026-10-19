import { Workspace } from './workspace.rs';
import type { InspectRequest, InspectResponse } from './inspection';

const workspace = new Workspace();

self.onmessage = ({ data }: MessageEvent<InspectRequest>) => {
	let response: InspectResponse;

	try {
		const result = workspace.compile(data.code);

		response = {
			type: 'result',
			id: data.id,
			compilation: {
				js_code: result.js_code,
				rust_code: result.rust_code,
				ts_code: result.ts_code,
				ambient_ts_code: result.ambient_ts_code,
				runtime_js_code: result.runtime_js_code,
				diagnostics: result.diagnostics,
				diagnostic_count: result.diagnostic_count
			}
		};
	} catch (cause) {
		response = {
			type: 'error',
			id: data.id,
			message: String(cause),
			fatal: cause instanceof WebAssembly.RuntimeError || cause instanceof RangeError
		};
	}

	self.postMessage(response);

	if (response.type === 'error' && response.fatal) self.close();
};

self.postMessage({ type: 'ready' } satisfies InspectResponse);
