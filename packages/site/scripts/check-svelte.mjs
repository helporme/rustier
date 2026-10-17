import { readdir } from 'node:fs/promises';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

let failed = false;

function run(command, args, capture = false) {
	const result = spawnSync(command, args, {
		stdio: capture ? ['ignore', 'pipe', 'inherit'] : 'inherit',
		encoding: 'utf8'
	});
	if (result.error) console.error(result.error);
	if (result.error || result.status !== 0) failed = true;
	if (capture && result.stdout) process.stdout.write(result.stdout);
	return result.stdout ?? '';
}

async function checkSvelte(directory) {
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const file = path.join(directory, entry.name);
		if (entry.isDirectory()) {
			await checkSvelte(file);
		} else if (entry.isFile() && entry.name.endsWith('.svelte')) {
			console.log(`\nSvelte autofixer: ${file}`);
			const output = run('npx', ['-y', '@sveltejs/mcp', 'svelte-autofixer', file, '--async'], true);
			// The CLI prints a report but exits successfully even when it finds issues.
			if (!/issues:\s*\[\s*\]/.test(output) || !/suggestions:\s*\[\s*\]/.test(output)) {
				failed = true;
			}
		}
	}
}

run('pnpm', ['exec', 'svelte-kit', 'sync']);
run('pnpm', ['exec', 'svelte-check', '--tsconfig', './tsconfig.json']);
await checkSvelte('src');

process.exitCode = failed ? 1 : 0;
