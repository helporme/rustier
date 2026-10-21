import { execFileSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import * as fs from 'node:fs/promises';
import path from 'node:path';

const [root, release] = process.argv.slice(2);
if (
	process.getuid() !== 0 ||
	!/^\/opt\/[\w-]+$/.test(root ?? '') ||
	!/^\d+-[a-f0-9]{12}$/.test(release ?? '')
)
	throw new Error('Invalid remote deployment invocation.');
const directory = path.join(root, 'releases', release);
const state = '/etc/rustier-site';
const config = path.join(root, 'deployment');
delete process.env.DOCKER_CONTEXT;
delete process.env.DOCKER_TLS_VERIFY;
delete process.env.DOCKER_CERT_PATH;
process.env.DOCKER_HOST = 'unix:///var/run/docker.sock';
const manifest = JSON.parse(await fs.readFile(path.join(directory, 'manifest.json'), 'utf8'));
if (
	manifest.release !== release ||
	manifest.directory !== root ||
	!/^rustier-site-compiler:[a-f0-9]{64}$/.test(manifest.compilerImage)
)
	throw new Error('Invalid release manifest.');

function run(command, args, options = {}) {
	return execFileSync(command, args, { stdio: 'inherit', ...options });
}
function capture(command, args, options = {}) {
	try {
		return run(command, args, {
			encoding: 'utf8',
			stdio: ['ignore', 'pipe', 'inherit'],
			...options
		}).trim();
	} catch {
		// A failed Compose snapshot may contain credentials in captured stdout.
		throw new Error(
			`${command} ${args[0]} failed; captured output omitted to protect credentials.`
		);
	}
}
function inspect(service) {
	try {
		return JSON.parse(
			capture('docker', ['inspect', `rustier-site-${service}-1`], { stdio: 'pipe' })
		)[0];
	} catch {
		return undefined;
	}
}
async function switchRelease(next) {
	const link = path.join(root, 'releases/current');
	if (!next) {
		await fs.rm(link, { force: true });
		return;
	}
	await fs.rm(`${link}.next`, { force: true });
	await fs.symlink(next, `${link}.next`);
	await fs.rename(`${link}.next`, link);
}
async function optional(file) {
	try {
		return await fs.readFile(file, 'utf8');
	} catch (error) {
		if (error.code === 'ENOENT') return undefined;
		throw error;
	}
}

const checksums = JSON.parse(await fs.readFile(path.join(directory, 'checksums.json'), 'utf8'));
for (const [file, checksum] of Object.entries(checksums)) {
	if (path.isAbsolute(file) || file.split('/').includes('..'))
		throw new Error('Invalid artifact path.');
	if (
		createHash('sha256')
			.update(await fs.readFile(path.join(directory, file)))
			.digest('hex') !== checksum
	)
		throw new Error(`Artifact checksum mismatch: ${file}`);
}
await fs.mkdir(state, { recursive: true, mode: 0o700 });
const tokenFile = path.join(state, 'compiler-token');
try {
	if ((await fs.lstat(tokenFile)).isSymbolicLink())
		throw new Error('Compiler token must not be a symlink.');
} catch (error) {
	if (error.code !== 'ENOENT') throw error;
}
let token = (await optional(tokenFile))?.trim();
if (!token) {
	token = randomBytes(32).toString('hex');
	await fs.writeFile(tokenFile, `${token}\n`, { mode: 0o600, flag: 'wx' });
}
if (!/^[a-f0-9]{64}$/.test(token)) throw new Error('Stored compiler token is invalid.');
await fs.chmod(tokenFile, 0o600);

const previousSite = inspect('site');
const previousCaddy = inspect('caddy');
const previousCompiler = inspect('compiler');
const previousRelease = await fs.readlink(path.join(root, 'releases/current')).catch((error) => {
	if (error.code === 'ENOENT') return undefined;
	throw error;
});
const env = {
	...process.env,
	COMPILER_TOKEN: token,
	COMPILER_IMAGE: manifest.compilerImage,
	SITE_DOMAIN: manifest.domain,
	DEPLOY_ROOT: root,
	CLOUDFLARE_TRUSTED_PROXIES: manifest.cloudflare.join(' '),
	SITE_RUNNER_VERSION: checksums['deployment/site-runner.mjs'],
	CADDY_CONFIG_VERSION: checksums['deployment/Caddyfile']
};
const compose = (...args) =>
	run(
		'docker',
		[
			'compose',
			'--env-file',
			'/dev/null',
			'-p',
			'rustier-site',
			'-f',
			path.join(config, 'compose.yaml'),
			...args
		],
		{ env }
	);

// A resolved Compose snapshot allows rollback even from the historical layout.
// It contains credentials and is stored only in the root-only server state directory.
const rollbackFile = path.join(state, 'rollback-compose.yaml');
const oldFiles = previousSite?.Config.Labels['com.docker.compose.project.config_files']?.split(',');
const configBackup = path.join(state, `config-${release}`);
if (oldFiles) {
	const oldEnvironment = Object.fromEntries(
		[...(previousCaddy?.Config.Env ?? []), ...(previousSite?.Config.Env ?? [])].map((value) => {
			const at = value.indexOf('=');
			return [value.slice(0, at), value.slice(at + 1)];
		})
	);
	const resolved = capture(
		'docker',
		[
			'compose',
			'--env-file',
			'/dev/null',
			'-p',
			'rustier-site',
			...oldFiles.flatMap((file) => ['-f', file]),
			'config'
		],
		{
			env: {
				...env,
				...oldEnvironment,
				COMPILER_IMAGE: previousCompiler?.Config.Image ?? manifest.compilerImage
			}
		}
	);
	await fs.writeFile(rollbackFile, resolved, { mode: 0o600 });
}
if (await optional(path.join(config, 'compose.yaml')))
	await fs.cp(config, configBackup, { recursive: true });

const compilerChanged = previousCompiler?.Config.Image !== manifest.compilerImage;
try {
	await fs.mkdir(config, { recursive: true, mode: 0o755 });
	await fs.cp(path.join(directory, 'deployment'), config, { recursive: true });
	compose('config', '--quiet');
	if (compilerChanged || previousCompiler?.State.Health?.Status !== 'healthy')
		compose(
			'up',
			'-d',
			'--no-deps',
			'--pull',
			'missing',
			'--wait',
			'--wait-timeout',
			'150',
			'compiler'
		);
	// Exercise the real sandbox, not just the HTTP listener. Never print its token.
	if (compilerChanged)
		run('docker', [
			'exec',
			'rustier-site-compiler-1',
			'node',
			'--input-type=module',
			'-e',
			`
const res = await fetch('http://127.0.0.1:3001/build', {
 method: 'POST', headers: {authorization: 'Bearer ' + process.env.COMPILER_TOKEN, 'content-type': 'application/json'},
 body: JSON.stringify({sessionId: crypto.randomUUID(), code: 'pub fn deployment_probe() -> u32 { 42 }',
 typescript: "import { deployment_probe } from './lib.rs'; console.log(deployment_probe())"}),
 signal: AbortSignal.timeout(145000)
});
const result = await res.json();
if (!res.ok || result.type !== 'ok' || !WebAssembly.validate(Buffer.from(result.wasm, 'base64')))
 throw new Error('Sandbox compilation probe failed: ' + (result.message ?? res.status));
`
		]);

	// Keep the preceding release's immutable assets available to already-open
	// pages, without carrying inherited assets forward indefinitely.
	if (previousRelease) {
		const previousDirectory = path.join(root, 'releases', previousRelease);
		const previousChecksums = JSON.parse(
			(await optional(path.join(previousDirectory, 'checksums.json'))) ?? '{}'
		);
		for (const file of Object.keys(previousChecksums)) {
			if (!file.startsWith('site/build/client/_app/immutable/') || file.split('/').includes('..'))
				continue;
			await fs.mkdir(path.dirname(path.join(directory, file)), { recursive: true });
			await fs.cp(path.join(previousDirectory, file), path.join(directory, file), {
				force: false,
				errorOnExist: false
			});
		}
	}
	await switchRelease(release);
	// Unchanged service definitions are a no-op. The runner version only changes
	// when its implementation changes, never when application files change.
	compose('up', '-d', '--no-deps', '--pull', 'missing', '--wait', '--wait-timeout', '90', 'site');
	run('docker', [
		'exec',
		'rustier-site-site-1',
		'node',
		'/deploy/deployment/site-runner.mjs',
		'activate',
		release
	]);
	compose('up', '-d', '--no-deps', '--pull', 'missing', '--wait', '--wait-timeout', '90', 'caddy');
	run('docker', [
		'exec',
		'rustier-site-site-1',
		'node',
		'-e',
		`
fetch('http://127.0.0.1:3000/playground').then(res => {
 if (!res.ok || res.headers.get('x-rustier-release') !== ${JSON.stringify(release)}) process.exit(1);
}).catch(() => process.exit(1));
`
	]);
	await fs.writeFile(
		path.join(state, 'deployment.json'),
		JSON.stringify({ ...manifest, checksums }),
		{ mode: 0o600 }
	);
	console.log(
		`Deployed ${release}; compiler ${compilerChanged ? 'updated' : 'unchanged'}; site container ${previousSite?.Id === inspect('site')?.Id ? 'preserved' : 'initialized/reconfigured'}.`
	);
} catch (error) {
	console.error('Deployment failed; restoring the previous release and service configuration.');
	await switchRelease(previousRelease);
	if (await optional(path.join(configBackup, 'compose.yaml')))
		await fs.cp(configBackup, config, { recursive: true });
	if (oldFiles) {
		run(
			'docker',
			[
				'compose',
				'--env-file',
				'/dev/null',
				'-p',
				'rustier-site',
				'-f',
				rollbackFile,
				'up',
				'-d',
				'--wait',
				'--wait-timeout',
				'150'
			],
			{ env }
		);
		if (
			previousRelease &&
			previousSite?.Config.Env.some((value) => value.startsWith('SITE_RUNNER_VERSION='))
		)
			run('docker', [
				'exec',
				'rustier-site-site-1',
				'node',
				'/deploy/deployment/site-runner.mjs',
				'activate',
				previousRelease
			]);
	} else compose('down');
	throw error;
} finally {
	await fs.rm(configBackup, { recursive: true, force: true });
}
