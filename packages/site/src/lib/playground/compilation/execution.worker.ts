import { EXECUTION_CONSOLE, type ExecutionMessage } from './execution-protocol';
import type { BuildResult } from './build.remote';

const send = (message: ExecutionMessage) => self.postMessage(message);

function format(value: unknown): string {
	if (typeof value === 'string') return value;
	if (typeof value === 'bigint') return `${value}n`;
	if (value instanceof Error) return `${value.name}: ${value.message}`;
	try {
		return (
			JSON.stringify(value, (_, item) => (typeof item === 'bigint' ? `${item}n` : item)) ??
			String(value)
		);
	} catch {
		return String(value);
	}
}

// User code runs off the UI thread and is terminated after each run or on timeout.
self.onmessage = async ({ data }: MessageEvent<Extract<BuildResult, { type: 'ok' }>>) => {
	const urls: string[] = [];
	const url = (part: BlobPart, type = 'text/javascript') => {
		const created = URL.createObjectURL(new Blob([part], { type }));
		urls.push(created);
		return created;
	};
	const output = Object.create(console) as Console;
	for (const level of ['log', 'info', 'debug', 'warn', 'error', 'dir', 'trace'] as const) {
		output[level] = (...values: unknown[]) =>
			send({ type: 'line', text: values.map(format).join(' ') });
	}
	output.assert = (condition, ...values: unknown[]) => {
		if (!condition) output.error('Assertion failed', ...values);
	};
	Object.assign(globalThis, { [EXECUTION_CONSOLE]: output });

	try {
		const binary = url(data.wasm, 'application/wasm');
		const glue = url(data.glue);
		const ffi = url(
			`import init from ${JSON.stringify(glue)};\nexport default () => init({ module_or_path: ${JSON.stringify(binary)} });\nexport * from ${JSON.stringify(glue)};`
		);
		const bundle = url(
			data.bundle.replaceAll(JSON.stringify(data.ffiModuleId), JSON.stringify(ffi))
		);
		await import(/* @vite-ignore */ bundle);
		send({ type: 'done' });
	} catch (cause) {
		send({ type: 'error', message: format(cause) });
	} finally {
		urls.forEach((created) => URL.revokeObjectURL(created));
	}
};
