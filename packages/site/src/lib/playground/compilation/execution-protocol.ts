// The bundler redirects user console calls here, leaving worker tooling untouched.
export const EXECUTION_CONSOLE = '__rustierConsole';

export type ExecutionMessage =
	{ type: 'line'; text: string } | { type: 'done' } | { type: 'error'; message: string };
