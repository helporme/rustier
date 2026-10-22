import { execFileSync, spawn } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import * as fs from 'node:fs/promises';
import { isIP } from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse } from 'yaml';

const repo = fileURLToPath(new URL('../../..', import.meta.url));
const deployment = fileURLToPath(new URL('./deploy', import.meta.url));
const sshOptions = [
	'-T',
	'-o',
	'BatchMode=yes',
	'-o',
	'StrictHostKeyChecking=yes',
	'-o',
	'ForwardAgent=no',
	'-o',
	'ConnectTimeout=10',
	'-o',
	'ServerAliveInterval=15',
	'-o',
	'ServerAliveCountMax=4'
];
const excluded =
	/^(node_modules|target|tests|snapshots|\.git|\.delta|\.rustier|\.svelte-kit|build|\.env.*)$|\.(md|node|swp|tsbuildinfo)$/;

function run(command, args, options = {}) {
	return execFileSync(command, args, { cwd: repo, stdio: 'inherit', ...options });
}

function capture(command, args, options = {}) {
	return run(command, args, {
		encoding: 'utf8',
		stdio: ['pipe', 'pipe', 'inherit'],
		...options
	}).trim();
}

function quote(value) {
	return `'${String(value).replaceAll("'", "'\\''")}'`;
}

async function files(directory, prefix = '', source = true) {
	const result = [];
	for (const entry of (await fs.readdir(directory, { withFileTypes: true })).sort((a, b) =>
		a.name.localeCompare(b.name)
	)) {
		if (source && excluded.test(entry.name)) continue;
		const name = path.posix.join(prefix, entry.name);
		if (entry.isSymbolicLink()) throw new Error(`Unexpected deployment symlink: ${name}`);
		if (entry.isDirectory())
			result.push(...(await files(path.join(directory, entry.name), name, source)));
		else if (entry.isFile()) result.push(name);
	}
	return result;
}

// Project the lockfile onto the compiler's dependency closure. Site-only dependency
// upgrades must not invalidate an otherwise identical compiler image.
export async function compilerFingerprint(architecture) {
	const lock = parse(await fs.readFile(path.join(repo, 'pnpm-lock.yaml'), 'utf8'));
	const selected = { settings: lock.settings, importers: {}, packages: {}, snapshots: {} };
	function visit(name, version) {
		if (version.startsWith('link:')) return;
		const key = `${name}@${version}`;
		if (selected.snapshots[key]) return;
		const snapshot = lock.snapshots[key];
		if (!snapshot) throw new Error(`Missing dependency in lockfile: ${key}`);
		selected.snapshots[key] = snapshot;
		selected.packages[key.split('(')[0]] = lock.packages[key.split('(')[0]];
		for (const [name, version] of Object.entries({
			...snapshot.dependencies,
			...snapshot.optionalDependencies
		}))
			visit(name, version);
	}
	for (const name of ['rustier', 'rustier-native', 'site-compiler-service']) {
		const key = `packages/${name}`;
		const importer = (selected.importers[key] = lock.importers[key]);
		for (const group of ['dependencies', 'devDependencies', 'optionalDependencies'])
			for (const [name, dependency] of Object.entries(importer[group] ?? {}))
				visit(name, dependency.version);
	}
	const hash = createHash('sha256').update(architecture).update(JSON.stringify(selected));
	for (const name of [
		'packages/rustier',
		'packages/rustier-native',
		'packages/rustier-ext',
		'packages/site-compiler-service',
		'rust-toolchain.toml',
		'pnpm-workspace.yaml'
	]) {
		const base = path.join(repo, name);
		const entries = (await fs.stat(base)).isDirectory() ? await files(base) : [''];
		for (const entry of entries) {
			// Rustier's generated output is rebuilt inside the image; the service bundle
			// is built locally and includes all its shared site-side contracts.
			if (
				(name !== 'packages/site-compiler-service' && entry.startsWith('dist/')) ||
				(name === 'packages/rustier-native' && /^rustier\.(js|d\.ts)$/.test(entry))
			)
				continue;
			const content = await fs.readFile(path.join(base, entry));
			hash.update(`${name}/${entry}\0${content.length}\0`).update(content);
		}
	}
	return hash.digest('hex');
}

async function transferImage(image, ssh) {
	const save = spawn('docker', ['image', 'save', image], { stdio: ['ignore', 'pipe', 'inherit'] });
	const gzip = spawn('gzip', ['-1'], { stdio: ['pipe', 'pipe', 'inherit'] });
	const upload = spawn('ssh', [...ssh, "bash -o pipefail -c 'gzip -dc | docker image load'"], {
		stdio: ['pipe', 'inherit', 'inherit']
	});
	save.stdout.pipe(gzip.stdin);
	gzip.stdout.pipe(upload.stdin);
	const children = [save, gzip, upload];
	try {
		await Promise.all(
			children.map(
				(child) =>
					new Promise((resolve, reject) => {
						child.once('error', reject);
						child.once('exit', (code, signal) =>
							code === 0 ? resolve() : reject(new Error(`Image transfer failed: ${code ?? signal}`))
						);
						child.stdin?.on('error', reject);
					})
			)
		);
	} finally {
		for (const child of children) if (child.exitCode === null) child.kill();
	}
}

async function main() {
	if (process.argv.includes('--help')) {
		console.log(
			'pnpm run deploy [--plan]\nEdit packages/site/deploy.json to select a trusted root SSH alias, hostname, and /opt directory.\n--plan builds the compiler bundle and checks image reuse, without modifying the server.\nNormal deployments build locally, provision Ubuntu/Debian if needed, and retain the previous release for rollback.'
		);
		return;
	}
	if (process.argv.slice(2).some((arg) => arg !== '--plan'))
		throw new Error('Unknown deploy argument; use --help.');
	const config = JSON.parse(await fs.readFile(new URL('../deploy.json', import.meta.url), 'utf8'));
	if (!/^[a-zA-Z0-9_][a-zA-Z0-9_.@:-]*$/.test(config.sshTarget ?? ''))
		throw new Error('Invalid sshTarget in deploy.json.');
	if (!/^(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}$/.test(config.domain ?? ''))
		throw new Error('Invalid domain in deploy.json.');
	if (!/^\/opt\/[a-zA-Z0-9_-]+$/.test(config.directory ?? ''))
		throw new Error('directory must be a direct child of /opt.');
	const ssh = [...sshOptions, config.sshTarget];
	const remote = (command, options) => capture('ssh', [...ssh, command], options);
	const architecture = remote('uname -m');
	const platform = { aarch64: 'linux/arm64', x86_64: 'linux/amd64' }[architecture];
	if (!platform) throw new Error(`Unsupported server architecture: ${architecture}`);
	run('pnpm', ['--filter', 'site-compiler-service', 'build']);
	const compilerImage = `rustier-site-compiler:${await compilerFingerprint(platform)}`;
	const compilerPresent =
		remote(
			`if docker image inspect ${quote(compilerImage)} >/dev/null 2>&1; then echo yes; else echo no; fi`
		) === 'yes';
	console.log(`Target: ${config.sshTarget} (${platform}), https://${config.domain}`);
	console.log(`Compiler: ${compilerPresent ? 'reuse' : 'build and upload'} ${compilerImage}`);
	if (process.argv.includes('--plan')) return;

	run('pnpm', ['--filter', 'site', 'build']);
	if (!compilerPresent) {
		try {
			capture('docker', ['image', 'inspect', compilerImage], { stdio: 'pipe' });
		} catch {
			run('docker', [
				'buildx',
				'build',
				'--platform',
				platform,
				'--load',
				'--tag',
				compilerImage,
				'--file',
				'packages/site-compiler-service/Dockerfile',
				'.'
			]);
		}
	}
	// Provisioning is separate from release activation. No running application
	// containers are touched while local compilation or image upload is in progress.
	remote('bash -s', { input: await fs.readFile(path.join(deployment, 'bootstrap.sh')) });
	remote(
		'if systemctl is-active --quiet rustier-ufw-rollback.timer; then systemctl stop rustier-ufw-rollback.timer; fi'
	);
	if (!compilerPresent) {
		// Docker image save needs temporary disk space. Release unused local build
		// cache before exporting, without pruning images, containers, or volumes.
		run('docker', ['builder', 'prune', '--force']);
		await transferImage(compilerImage, ssh);
	}
	const release = `${Date.now()}-${randomBytes(6).toString('hex')}`;
	const staging = path.join(repo, 'packages/site/.deploy', release);
	await fs.mkdir(path.join(staging, 'site'), { recursive: true });
	try {
		await fs.cp(path.join(repo, 'packages/site/build'), path.join(staging, 'site/build'), {
			recursive: true
		});
		await fs.writeFile(path.join(staging, 'site/package.json'), '{"type":"module"}\n');
		await fs.cp(deployment, path.join(staging, 'deployment'), { recursive: true });
		const cloudflare = [];
		for (const version of [4, 6]) {
			const response = await fetch(`https://www.cloudflare.com/ips-v${version}`, {
				signal: AbortSignal.timeout(15000)
			});
			if (!response.ok) throw new Error('Could not load official Cloudflare proxy ranges.');
			for (const range of (await response.text()).trim().split(/\s+/)) {
				const [address, prefix] = range.split('/');
				if (
					isIP(address) !== version ||
					!/^\d+$/.test(prefix) ||
					Number(prefix) > (version === 4 ? 32 : 128)
				)
					throw new Error(`Invalid Cloudflare proxy range: ${range}`);
				cloudflare.push(range);
			}
		}
		await fs.writeFile(
			path.join(staging, 'manifest.json'),
			JSON.stringify({ ...config, release, compilerImage, cloudflare })
		);
		const checksums = {};
		for (const file of await files(staging, '', false))
			checksums[file] = createHash('sha256')
				.update(await fs.readFile(path.join(staging, file)))
				.digest('hex');
		await fs.writeFile(path.join(staging, 'checksums.json'), JSON.stringify(checksums));
		remote(`mkdir -p ${quote(`${config.directory}/releases/${release}`)}`);
		run('rsync', [
			'-rltz',
			'--safe-links',
			'--chmod=Du=rwx,Dgo=rx,Fu=rw,Fgo=r',
			'-e',
			['ssh', ...sshOptions].join(' '),
			`${staging}/`,
			`${config.sshTarget}:${config.directory}/releases/${release}/`
		]);
		run('ssh', [
			...ssh,
			`flock -n /etc/rustier-site/deploy.lock node ${quote(`${config.directory}/releases/${release}/deployment/remote.mjs`)} ${quote(config.directory)} ${quote(release)}`
		]);
		const response = await fetch(`https://${config.domain}/playground`, {
			signal: AbortSignal.timeout(30000),
			cache: 'no-store'
		});
		if (!response.ok || response.headers.get('x-rustier-release') !== release)
			throw new Error(
				'Public verification failed. The server-side health check passed; inspect the proxy/DNS before deploying again.'
			);
		console.log(`Verified https://${config.domain}/playground — release ${release}`);
	} finally {
		await fs.rm(staging, { recursive: true, force: true });
	}
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	main().catch((error) => {
		console.error(`deploy: ${error.message}`);
		process.exitCode = 1;
	});
}
