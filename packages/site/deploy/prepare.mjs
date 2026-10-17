import { randomBytes } from 'node:crypto';
import { writeFile } from 'node:fs/promises';

const environment = new URL('../../../.env', import.meta.url);
const content = [
	'COMPILER_HOST=127.0.0.1',
	'COMPILER_PORT=3001',
	'COMPILER_URL=http://127.0.0.1:3001',
	`COMPILER_TOKEN=${randomBytes(32).toString('hex')}`,
	''
].join('\n');

try {
	await writeFile(environment, content, { flag: 'wx', mode: 0o600 });
	console.log('Created root .env for the site and compiler service.');
} catch (error) {
	if (error.code !== 'EEXIST') throw error;

	console.log('Keeping existing root .env.');
}
