import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';

// Share an ephemeral credential with both processes without writing it to disk.
const child = spawn('pnpm', ['run', '--parallel', '/^dev\\//'], {
	stdio: 'inherit',
	env: {
		...process.env,
		COMPILER_URL:
			process.env.COMPILER_URL || `http://127.0.0.1:${process.env.COMPILER_PORT || 3001}`,
		COMPILER_TOKEN: process.env.COMPILER_TOKEN || randomBytes(32).toString('hex')
	}
});

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
