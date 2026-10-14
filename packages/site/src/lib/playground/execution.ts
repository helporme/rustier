export type ExecutionLevel = 'log' | 'info' | 'debug' | 'warn' | 'error';

export interface ExecutionLine {
	level: ExecutionLevel;
	text: string;
}

let executions = 0;

export async function execute(
	code: string,
	modules: Record<string, string>,
	onLine: (line: ExecutionLine) => void
) {
	const key = `__rustier_console_${executions++}`;
	const globals = globalThis as unknown as Record<string, unknown>;
	const write =
		(level: ExecutionLevel) =>
		(...values: unknown[]) =>
			onLine({ level, text: values.map((value) => format(value)).join(' ') });

	globals[key] = Object.assign(Object.create(console), {
		log: write('log'),
		info: write('info'),
		debug: write('debug'),
		warn: write('warn'),
		error: write('error'),
		dir: write('log'),
		trace: write('log'),
		assert: (condition: unknown, ...values: unknown[]) => {
			if (!condition) write('error')('Assertion failed', ...values);
		}
	});

	let source = `const console = globalThis[${JSON.stringify(key)}];\n${code}`;

	for (const [specifier, url] of Object.entries(modules)) {
		for (const quote of ['"', "'"]) {
			source = source.replaceAll(`${quote}${specifier}${quote}`, JSON.stringify(url));
		}
	}

	const url = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }));

	try {
		await import(/* @vite-ignore */ url);
	} catch (error) {
		onLine({ level: 'error', text: `Uncaught ${format(error)}` });
	} finally {
		URL.revokeObjectURL(url);
		delete globals[key];
	}
}

export function format(value: unknown, depth = 0, seen = new Set<object>()): string {
	if (typeof value === 'string') return depth === 0 ? value : JSON.stringify(value);
	if (typeof value === 'bigint') return `${value}n`;
	if (typeof value === 'function') return `[Function ${value.name || '(anonymous)'}]`;
	if (typeof value !== 'object' || value === null) return String(value);
	if (value instanceof Error) return `${value.name}: ${value.message}`;
	if (seen.has(value)) return '[Circular]';
	if (depth > 3) return Array.isArray(value) ? '[Array]' : '[Object]';

	seen.add(value);

	const nested = (item: unknown) => format(item, depth + 1, seen);
	const text = Array.isArray(value)
		? `[${value.map(nested).join(', ')}]`
		: value instanceof Map
			? `Map(${value.size}) { ${[...value].map(([key, item]) => `${nested(key)} => ${nested(item)}`).join(', ')} }`
			: value instanceof Set
				? `Set(${value.size}) { ${[...value].map(nested).join(', ')} }`
				: formatObject(value, nested);

	seen.delete(value);

	return text;
}

function formatObject(value: object, nested: (item: unknown) => string) {
	const prototype = Object.getPrototypeOf(value);
	const name = prototype?.constructor?.name;
	const accessors =
		prototype && prototype !== Object.prototype
			? Object.entries(Object.getOwnPropertyDescriptors(prototype))
					.filter(([, descriptor]) => descriptor.get)
					.map(([key]) => key)
			: [];
	const entries = [...new Set([...Object.keys(value), ...accessors])].map((key) => {
		let item: unknown;

		try {
			item = (value as Record<string, unknown>)[key];
		} catch (error) {
			item = error;
		}

		return `${key}: ${nested(item)}`;
	});
	const prefix = name && name !== 'Object' ? `${name} ` : '';

	return entries.length > 0 ? `${prefix}{ ${entries.join(', ')} }` : `${prefix}{}`;
}
