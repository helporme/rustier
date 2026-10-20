import { spawn } from 'node:child_process';

if (!process.env.COMPILER_TOKEN || Buffer.byteLength(process.env.COMPILER_TOKEN) < 32) {
	throw new Error(
		'Set COMPILER_TOKEN (at least 32 bytes) in the root .env for separate dev/compiler and dev/site sessions, or use pnpm dev to generate a shared temporary token.'
	);
}

const child = spawn(
	'pnpm',
	[
		'exec',
		'tsdown',
		'--watch',
		'--on-success',
		'docker compose up --build --watch --remove-orphans'
	],
	{ cwd: import.meta.dirname, stdio: 'inherit' }
);
for (const signal of ['SIGINT', 'SIGTERM']) {
	process.once(signal, () => child.kill(signal));
}
child.once('error', (error) => {
	console.error(error);
	process.exitCode = 1;
});
child.once('exit', (code, signal) => {
	process.exitCode = code ?? (signal === 'SIGINT' ? 130 : 1);
});
