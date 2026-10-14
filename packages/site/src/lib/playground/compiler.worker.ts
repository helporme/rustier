import { Workspace } from './workspace.rs';
import type { CompilerRequest, CompilerMessage, Compilation } from './compilation';

const workspace = new Workspace();

self.onmessage = ({ data }: MessageEvent<CompilerRequest>) => {
	let response: CompilerMessage;

	try {
		if ('type' in data && data.type === 'hover') {
			const result = workspace.hover(data.code, data.offset);
			const markdown = result.markdown;

			response = {
				type: 'hover',
				id: data.id,
				hover: markdown ? { markdown, start: result.start, end: result.end } : undefined
			};
		} else if ('type' in data && data.type === 'definition') {
			const result = workspace.definition(data.code, data.offset);

			response = {
				type: 'definition',
				id: data.id,
				definition: result.found ? { start: result.start, end: result.end } : undefined
			};
		} else {
			const result = workspace.compile(data.code);
			const markers = result.rust_markers;
			const rust_markers: Compilation['rust_markers'] = [];

			for (let index = markers.count(); index > 0; index--) {
				const marker = markers.take();

				rust_markers.push({
					start: marker.start,
					end: marker.end,
					severity: marker.severity,
					message: marker.message,
					code: marker.code
				});
			}

			response = {
				id: data.id,
				compilation: {
					js_code: result.js_code,
					rust_code: result.rust_code,
					ts_code: result.ts_code,
					ambient_ts_code: result.ambient_ts_code,
					ts_sourcemap: result.ts_sourcemap,
					diagnostics: result.diagnostics,
					diagnostic_count: result.diagnostic_count,
					rust_markers
				}
			};
		}
	} catch (error) {
		response = {
			id: data.id,
			error: String(error),
			fatal: error instanceof WebAssembly.RuntimeError || error instanceof RangeError
		};
	}

	self.postMessage(response);

	if ('error' in response && response.fatal) self.close();
};

self.postMessage({ type: 'ready' } satisfies CompilerMessage);
