import { spawn, spawnSync } from 'node:child_process';
import { access } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const environment = new URL('../../../.env', import.meta.url);

try {
	await access(environment);
} catch {
	throw new Error('Run pnpm prepare before pnpm dev/compiler.');
}

const security = spawnSync('docker', ['info', '--format', '{{json .SecurityOptions}}'], {
	encoding: 'utf8'
});

if (security.error) throw security.error;
if (security.status !== 0) {
	process.stderr.write(security.stderr);
	process.exit(security.status ?? 1);
}

const args = [
	'compose',
	'--project-directory',
	fileURLToPath(new URL('.', import.meta.url)),
	'--env-file',
	fileURLToPath(environment),
	'-p',
	'rustier-site-dev',
	'-f',
	fileURLToPath(new URL('compose.yaml', import.meta.url)),
	'-f',
	fileURLToPath(new URL('compose.dev.yaml', import.meta.url))
];

if (JSON.parse(security.stdout).includes('name=apparmor')) {
	args.push('-f', fileURLToPath(new URL('compose.apparmor.yaml', import.meta.url)));
}

args.push('up', '--build', 'compiler');

const compiler = spawn('docker', args, { stdio: 'inherit' });

compiler.once('error', (error) => {
	console.error(error.message);
	process.exitCode = 1;
});
compiler.once('exit', (code) => {
	process.exitCode = code ?? 1;
});
