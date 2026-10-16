import { mkdir, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { examples } from '../src/lib/playground/examples.ts';

const root = path.resolve('.rustier/example-build');
const output = path.resolve('.rustier/example-results');

process.env.PLAYGROUND_ROOT = root;

await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });

const { compile } = await import('../../site-compiler-service/src/compilation/compile.ts');

for (const example of examples) {
	const result = await compile({ code: example.code });

	if (result.type !== 'ok') throw new Error(`${example.name}: ${result.message}`);

	await writeFile(
		path.join(output, `${example.slug}.json`),
		JSON.stringify({ code: example.code, result })
	);
	console.log(`Cached ${example.name}`);
}
