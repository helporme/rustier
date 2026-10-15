#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import * as fs from 'node:fs/promises';
import { isIP } from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = '/opt/rustier-site';
const stateRoot = '/etc/rustier-site';
const script = fileURLToPath(import.meta.url);
const sshOptions = [
	'-T',
	'-o',
	'BatchMode=yes',
	'-o',
	'StrictHostKeyChecking=yes',
	'-o',
	'ForwardAgent=no',
	'-o',
	'ControlMaster=no',
	'-o',
	'ControlPath=none',
	'-o',
	'ConnectTimeout=10',
	'-o',
	'ServerAliveInterval=15',
	'-o',
	'ServerAliveCountMax=4'
];
const configFiles = [
	'compose.yaml',
	'compose.production.yaml',
	'compose.apparmor.yaml',
	'Caddyfile',
	'compiler.apparmor'
];
const excluded =
	/^(?:node_modules|target|tests|dist|build|\.git|\.delta|\.svelte-kit|\.rustier|\.DS_Store|\.env(?:\..*)?|\.npmrc|\.ssh|credentials(?:\.toml)?)$|\.(?:node|swp|tsbuildinfo|pem|key)$/;

function run(command, args, options = {}) {
	return execFileSync(command, args, { stdio: 'inherit', ...options });
}

function capture(command, args, options = {}) {
	return run(command, args, {
		stdio: ['pipe', 'pipe', 'inherit'],
		encoding: 'utf8',
		...options
	}).trim();
}

function quote(value) {
	return `'${String(value).replaceAll("'", "'\\''")}'`;
}

function validateDomain(domain) {
	if (
		!domain ||
		domain.length > 253 ||
		!/^([a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z][a-zA-Z0-9-]{0,61}[a-zA-Z0-9]$/.test(
			domain
		)
	)
		throw new Error('Expected a public hostname, without a scheme, port, wildcard or path.');
}

function validateCloudflare(ranges) {
	for (const version of [4, 6]) {
		const entries = ranges?.[`ipv${version}`];
		if (!Array.isArray(entries) || entries.length < 1 || entries.length > 100)
			throw new Error(`Missing or invalid Cloudflare IPv${version} ranges.`);

		for (const entry of entries) {
			if (typeof entry !== 'string') throw new Error('Invalid Cloudflare address range.');
			const parts = entry.split('/');
			const prefix = Number(parts[1]);
			if (
				parts.length !== 2 ||
				isIP(parts[0]) !== version ||
				!/^[1-9]\d{0,2}$/.test(parts[1]) ||
				prefix < (version === 4 ? 8 : 16) ||
				prefix > (version === 4 ? 32 : 128)
			)
				throw new Error(`Invalid Cloudflare address range: ${entry}`);
		}
	}
	return ranges;
}

async function cloudflareRanges() {
	const entries = await Promise.all(
		[4, 6].map(async (version) => {
			const response = await fetch(`https://www.cloudflare.com/ips-v${version}`, {
				signal: AbortSignal.timeout(15_000)
			});
			if (!response.ok)
				throw new Error(`Cloudflare IPv${version} list returned HTTP ${response.status}`);
			const ranges = [...new Set((await response.text()).trim().split(/\s+/))].sort();
			return [`ipv${version}`, ranges];
		})
	);
	return validateCloudflare(Object.fromEntries(entries));
}

async function readJSON(file, fallback) {
	try {
		return JSON.parse(await fs.readFile(file, 'utf8'));
	} catch (error) {
		if (error.code === 'ENOENT') return fallback;
		throw error;
	}
}

async function atomicWrite(file, content, mode = 0o600) {
	const temporary = `${file}.${process.pid}.tmp`;
	await fs.writeFile(temporary, content, { mode });
	await fs.rename(temporary, file);
}

async function switchRelease(release) {
	const temporary = `${root}/releases/current.${process.pid}`;
	await fs.symlink(release, temporary);
	await fs.rename(temporary, `${root}/releases/current`);
}

async function hashFiles(base, entries, hash = createHash('sha256')) {
	for (const entry of [...entries].sort()) {
		if (excluded.test(path.basename(entry))) continue;
		const file = path.join(base, entry);
		const stat = await fs.lstat(file);
		if (stat.isSymbolicLink()) throw new Error(`Unexpected source symlink: ${file}`);
		if (stat.isDirectory()) {
			await hashFiles(
				base,
				(await fs.readdir(file)).map((name) => path.join(entry, name)),
				hash
			);
		} else if (stat.isFile()) {
			const contents = await fs.readFile(file);
			hash.update(`${entry}\0${contents.length}\0`).update(contents);
		}
	}
	return hash;
}

async function compilerFingerprint(repo, architecture) {
	const { parse } = await import('yaml');
	const lock = parse(await fs.readFile(path.join(repo, 'pnpm-lock.yaml'), 'utf8'));
	const selected = { settings: lock.settings, importers: {}, packages: {}, snapshots: {} };
	const visitPackage = (name, version) => {
		if (version.startsWith('link:')) return;
		const key = `${name}@${version}`;
		if (selected.snapshots[key]) return;
		const snapshot = lock.snapshots[key];
		if (!snapshot) throw new Error(`Unsupported lockfile dependency: ${key}`);
		selected.snapshots[key] = snapshot;
		const packageKey = key.split('(')[0];
		selected.packages[packageKey] = lock.packages[packageKey];
		for (const [dependency, resolved] of Object.entries({
			...snapshot.dependencies,
			...snapshot.optionalDependencies
		}))
			visitPackage(dependency, resolved);
	};
	for (const importer of [
		'packages/rustier',
		'packages/rustier-native',
		'packages/site-compiler-service'
	]) {
		selected.importers[importer] = lock.importers[importer];
		for (const entry of ['dependencies', 'devDependencies', 'optionalDependencies'])
			for (const [name, dependency] of Object.entries(lock.importers[importer][entry] ?? {}))
				visitPackage(name, dependency.version);
	}
	const dockerfile = await fs.readFile(path.join(repo, 'packages/site/Dockerfile'), 'utf8');
	const compilerStages = dockerfile
		.replace(/FROM toolchain AS site-build[\s\S]*?(?=FROM toolchain AS compiler-build)/, '')
		.replace(
			/FROM \$\{NODE_IMAGE\} AS site\n[\s\S]*?(?=FROM \$\{NODE_IMAGE\} AS worker-runtime)/,
			''
		);
	const hash = createHash('sha256')
		.update(architecture)
		.update(compilerStages)
		.update(JSON.stringify(selected));
	await hashFiles(
		repo,
		[
			'packages/rustier',
			'packages/rustier-native',
			'packages/rustier-rt',
			'packages/site-compiler-service',
			'rust-toolchain.toml',
			'pnpm-workspace.yaml',
			'package.json',
			'packages/site/Cargo.toml',
			'packages/site/Cargo.lock',
			'packages/site/build.rs',
			'packages/site/.cargo',
			'packages/site/src/lib.rs',
			'packages/site/src/lib/playground/compilation-schema.ts',
			'packages/site/Dockerfile.dockerignore'
		],
		hash
	);
	return hash.digest('hex');
}

async function localDeploy(target, domain = 'rustier.dev') {
	if (!target || target.startsWith('-') || !/^[a-zA-Z0-9_.@:-]+$/.test(target))
		throw new Error('Invalid SSH destination; SSH config aliases are supported.');
	validateDomain(domain);
	capture('rsync', ['--version']);
	const cloudflare = await cloudflareRanges();
	const repo = path.resolve(path.dirname(script), '../../..');
	const ssh = (command, options = {}) => capture('ssh', [...sshOptions, target, command], options);
	const source = await fs.readFile(script);
	ssh('bash -s', {
		input: `
set -euo pipefail
[[ $EUID == 0 ]] || { echo 'Root SSH access is required.' >&2; exit 1; }
. /etc/os-release
[[ \${ID:-} == ubuntu && \${VERSION_ID:-} =~ ^[0-9]{2}\\.[0-9]{2}$ ]] &&
  dpkg --compare-versions "$VERSION_ID" ge 24.04 || { echo 'Ubuntu 24.04 or newer is required.' >&2; exit 1; }
if ! command -v node >/dev/null || ! command -v rsync >/dev/null; then
  if command -v cloud-init >/dev/null; then
    timeout 300 cloud-init status --wait >&2 || [[ $? == 2 ]]
  fi
  apt-get update >&2
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends nodejs rsync >&2
fi
`
	});
	const host = JSON.parse(ssh('node --input-type=module - --host-info', { input: source }));
	const compilerImage = `rustier-site-compiler:${await compilerFingerprint(repo, host.architecture)}`;
	host.compilerPresent = JSON.parse(
		ssh(`node --input-type=module - --host-info ${quote(compilerImage)}`, { input: source })
	).compilerPresent;
	const compilerChanged = !host.compilerPresent;
	const release = `${Date.now()}-${randomBytes(6).toString('hex')}`;
	const temporary = await fs.mkdtemp(path.join(os.tmpdir(), 'rustier-deploy-'));
	try {
		console.log('Building the site with local Vite; the running site and compiler are untouched.');
		run('pnpm', ['--filter', 'site', 'build'], { cwd: repo });
		await fs.mkdir(`${temporary}/site`);
		await fs.cp(`${repo}/packages/site/build`, `${temporary}/site/build`, { recursive: true });
		await fs.writeFile(`${temporary}/site/package.json`, JSON.stringify({ type: 'module' }));

		for (const file of await fs.readdir(`${temporary}/site`, { recursive: true })) {
			if (/\.(?:node|dylib|so(?:\.\d+)*)$/.test(file))
				throw new Error(
					`Site output contains a native binary that cannot be deployed portably: ${file}`
				);
		}

		await fs.mkdir(`${temporary}/config`);
		for (const file of configFiles)
			await fs.copyFile(`${repo}/packages/site/${file}`, `${temporary}/config/${file}`);
		const configHash = (await hashFiles(temporary, ['config']))
			.update(JSON.stringify(cloudflare))
			.digest('hex');
		await fs.writeFile(
			`${temporary}/manifest.json`,
			JSON.stringify({ release, domain, compilerImage, configHash, cloudflare })
		);
		await fs.copyFile(script, `${temporary}/deploy.mjs`);
		if (compilerChanged) {
			console.log('Compiler inputs changed (or image missing); building its image locally.');
			run(
				'docker',
				[
					'buildx',
					'build',
					'--platform',
					`linux/${host.architecture}`,
					'-f',
					'packages/site/Dockerfile',
					'--target',
					'compiler',
					'--load',
					'-t',
					compilerImage,
					'.'
				],
				{
					cwd: repo
				}
			);
			run('docker', ['save', '-o', `${temporary}/compiler.tar`, compilerImage]);
		} else {
			console.log('Compiler unchanged: reusing its image and running workers.');
		}
		if (
			`rustier-site-compiler:${await compilerFingerprint(repo, host.architecture)}` !==
			compilerImage
		)
			throw new Error('Compiler inputs changed during the build; rerun deployment.');

		console.log('Uploading changed artifacts only; unchanged files reuse the previous release.');
		await fs.chmod(temporary, 0o755);
		ssh(`node --input-type=module - --prepare ${quote(release)}`, { input: source });
		const rsyncTarget = target.includes(':') ? target.replace(/(^|@)([^@]+)$/, '$1[$2]') : target;
		run('rsync', [
			'-azc',
			'--no-owner',
			'--no-group',
			'--chmod=Du=rwx,Dgo=rx,Fu=rw,Fgo=r',
			'--stats',
			...(host.state ? [`--link-dest=${root}/releases/${host.state.release}`] : []),
			'-e',
			['ssh', ...sshOptions].map(quote).join(' '),
			'--',
			`${temporary}/`,
			`${rsyncTarget}:${root}/releases/${release}/`
		]);
		const remote = `${root}/releases/${release}/deploy.mjs`;
		const locked = (mode) =>
			run('ssh', [
				...sshOptions,
				target,
				`flock -n ${quote(`${stateRoot}/deploy.lock`)} node ${quote(remote)} ${mode} ${quote(release)}`
			]);
		try {
			locked('--firewall');
			locked('--confirm-firewall');
		} catch (error) {
			throw new Error(
				`Firewall setup or fresh SSH verification failed. Any unconfirmed rule change rolls back within two minutes. ${error.message}`,
				{ cause: error }
			);
		}
		locked('--remote');
	} finally {
		await fs.rm(temporary, { recursive: true, force: true });
	}
}

async function hostInfo(compilerImage) {
	if (process.getuid() !== 0) throw new Error('Root SSH access is required.');
	const release = Object.fromEntries(
		(await fs.readFile('/etc/os-release', 'utf8'))
			.split('\n')
			.filter((line) => /^[A-Z_]+=/.test(line))
			.map((line) => {
				const index = line.indexOf('=');
				return [line.slice(0, index), line.slice(index + 1).replace(/^"|"$/g, '')];
			})
	);
	if (
		release.ID !== 'ubuntu' ||
		!/^\d{2}\.\d{2}$/.test(release.VERSION_ID) ||
		Number(release.VERSION_ID) < 24.04 ||
		!/^[a-z]+$/.test(release.VERSION_CODENAME)
	)
		throw new Error('Ubuntu 24.04 or newer with a valid release codename is required.');
	const architecture = capture('dpkg', ['--print-architecture']);
	if (!['arm64', 'amd64'].includes(architecture))
		throw new Error('Only ARM64 and AMD64 are supported.');
	const state = await readJSON(`${stateRoot}/deployment.json`, null);
	let compilerPresent = false;
	const image = compilerImage ?? state?.compilerImage;
	if (image) {
		if (!/^rustier-site-compiler:[a-f0-9]{64}$/.test(image))
			throw new Error('Invalid compiler image reference.');
		try {
			capture('docker', ['image', 'inspect', image], { stdio: 'pipe' });
			compilerPresent = true;
		} catch {
			/* A missing image must be uploaded again. */
		}
	}
	return { architecture, codename: release.VERSION_CODENAME, state, compilerPresent };
}

async function prepare(release) {
	await hostInfo();
	for (const directory of [root, `${root}/releases`, stateRoot]) {
		try {
			if ((await fs.lstat(directory)).isSymbolicLink())
				throw new Error(`Deployment root must not be a symlink: ${directory}`);
		} catch (error) {
			if (error.code !== 'ENOENT') throw error;
		}
		await fs.mkdir(directory, {
			recursive: true,
			mode: directory === `${root}/releases` ? 0o755 : 0o700
		});
		await fs.chmod(directory, directory === `${root}/releases` ? 0o755 : 0o700);
	}
	try {
		if ((await fs.lstat(`${stateRoot}/deploy.lock`)).isSymbolicLink())
			throw new Error('Deployment lock must not be a symlink.');
	} catch (error) {
		if (error.code !== 'ENOENT') throw error;
	}
	await fs.mkdir(`${root}/releases/${release}`, { mode: 0o755 });
}

function installDocker(host) {
	try {
		capture('docker', ['info'], { stdio: 'pipe' });
	} catch {
		try {
			capture('docker', ['--version'], { stdio: 'pipe' });
			throw new Error('Docker is installed but unavailable; fix the daemon before deploying.');
		} catch (error) {
			if (error.code !== 'ENOENT') throw error;
		}
		run('apt-get', ['update']);
		run(
			'apt-get',
			['install', '-y', '--no-install-recommends', 'ca-certificates', 'curl', 'gnupg'],
			{ env: { ...process.env, DEBIAN_FRONTEND: 'noninteractive' } }
		);
		run('curl', [
			'-fsSI',
			'--connect-timeout',
			'10',
			'--max-time',
			'30',
			`https://download.docker.com/linux/ubuntu/dists/${host.codename}/stable/binary-${host.architecture}/Packages.gz`
		]);
		for (const name of ['containerd', 'runc', 'podman-docker']) {
			let installed = false;
			try {
				installed =
					capture('dpkg-query', ['-W', '-f=${Status}', name], { stdio: 'pipe' }) ===
					'install ok installed';
			} catch {
				/* Package is absent. */
			}
			if (installed) throw new Error(`Refusing to replace existing container runtime: ${name}`);
		}
		run('install', ['-d', '-m', '755', '/etc/apt/keyrings']);
		const key = capture('curl', ['-fsSL', 'https://download.docker.com/linux/ubuntu/gpg']);
		const fingerprints = capture('gpg', ['--batch', '--show-keys', '--with-colons'], {
			input: key,
			stdio: ['pipe', 'pipe', 'inherit']
		});
		if (
			fingerprints
				.split('\n')
				.find((line) => line.startsWith('fpr:'))
				?.split(':')[9] !== '9DC858229FC7DD38854AE2D88D81803C0EBFCD88'
		)
			throw new Error('Unexpected Docker signing key.');
		run('tee', ['/etc/apt/keyrings/rustier-docker.asc'], {
			input: key,
			stdio: ['pipe', 'ignore', 'inherit']
		});
		run('chmod', ['644', '/etc/apt/keyrings/rustier-docker.asc']);
		run('tee', ['/etc/apt/sources.list.d/rustier-docker.list'], {
			input: `deb [arch=${host.architecture} signed-by=/etc/apt/keyrings/rustier-docker.asc] https://download.docker.com/linux/ubuntu ${host.codename} stable\n`,
			stdio: ['pipe', 'ignore', 'inherit']
		});
		run('apt-get', ['update']);
		run(
			'apt-get',
			[
				'install',
				'-y',
				'docker-ce',
				'docker-ce-cli',
				'containerd.io',
				'docker-buildx-plugin',
				'docker-compose-plugin'
			],
			{ env: { ...process.env, DEBIAN_FRONTEND: 'noninteractive' } }
		);
		run('systemctl', ['enable', '--now', 'docker']);
	}
	run('docker', ['compose', 'version']);
	try {
		capture('curl', ['--version'], { stdio: 'pipe' });
	} catch {
		run('apt-get', ['update']);
		run('apt-get', ['install', '-y', '--no-install-recommends', 'ca-certificates', 'curl'], {
			env: { ...process.env, DEBIAN_FRONTEND: 'noninteractive' }
		});
	}
}

function firewallRules(cloudflare, interfaces, ports) {
	validateCloudflare(cloudflare);
	if (!interfaces.length || interfaces.some((name) => !/^[a-zA-Z0-9_.:-]{1,15}$/.test(name)))
		throw new Error('Cannot identify the public network interfaces.');
	if (!ports.length || ports.some((port) => !Number.isInteger(port) || port < 1 || port > 65535))
		throw new Error('Cannot identify the SSH listening ports.');

	return `add table inet rustier_site
delete table inet rustier_site
table inet rustier_site {
	set cloudflare4 { type ipv4_addr; flags interval; elements = { ${cloudflare.ipv4.join(', ')} }; }
	set cloudflare6 { type ipv6_addr; flags interval; elements = { ${cloudflare.ipv6.join(', ')} }; }
	set public_interfaces { type ifname; elements = { ${interfaces.map((name) => JSON.stringify(name)).join(', ')} }; }
	chain input {
		type filter hook input priority 0; policy drop;
		iifname "lo" counter accept
		ct state invalid counter drop
		ct state established,related counter accept
		meta l4proto { icmp, ipv6-icmp } counter accept
		iifname @public_interfaces udp sport 67 udp dport 68 counter accept
		iifname @public_interfaces udp sport 547 udp dport 546 counter accept
		tcp dport { ${ports.join(', ')} } counter accept
		ip saddr @cloudflare4 tcp dport { 80, 443 } counter accept
		ip6 saddr @cloudflare6 tcp dport { 80, 443 } counter accept
		counter drop
	}
	chain forward {
		type filter hook forward priority -10; policy accept;
		iifname @public_interfaces ct state invalid counter drop
		iifname @public_interfaces ct state established,related counter accept
		iifname @public_interfaces ip saddr @cloudflare4 meta l4proto tcp ct status dnat ct original proto-dst { 80, 443 } counter accept
		iifname @public_interfaces ip6 saddr @cloudflare6 meta l4proto tcp ct status dnat ct original proto-dst { 80, 443 } counter accept
		iifname @public_interfaces counter drop
	}
}
`;
}

async function rollbackFirewall(release) {
	if (process.getuid() !== 0) throw new Error('Firewall operations require root.');
	const pending = await readJSON(`${stateRoot}/firewall-pending.json`, null);
	if (!pending || pending.release !== release) return;
	run('nft', ['--file', pending.rollback]);
	if (!pending.enabled) {
		try {
			run('systemctl', ['disable', '--now', 'rustier-firewall.service']);
		} catch {
			/* The first deployment may not have created the unit yet. */
		}
	}
	for (const [file, previous, mode] of [
		[`${stateRoot}/firewall.nft`, pending.previousConfig, 0o600],
		['/etc/systemd/system/rustier-firewall.service', pending.previousUnit, 0o644]
	]) {
		if (previous === null) await fs.rm(file, { force: true });
		else await atomicWrite(file, previous, mode);
	}
	run('systemctl', ['daemon-reload']);
	if (pending.enabled) run('systemctl', ['enable', '--now', 'rustier-firewall.service']);
	await fs.rm(`${stateRoot}/firewall-pending.json`);
	await fs.rm(pending.rollback, { force: true });
	await fs.rm(`${stateRoot}/firewall-candidate.nft`, { force: true });
	console.error('Unconfirmed firewall change rolled back.');
}

async function enforceFirewall(release) {
	const host = await hostInfo();
	installDocker(host);
	const manifest = await readJSON(`${root}/releases/${release}/manifest.json`);
	if (manifest.release !== release) throw new Error('Invalid firewall release.');
	validateCloudflare(manifest.cloudflare);
	const existing = await readJSON(`${stateRoot}/firewall-pending.json`, null);
	if (existing)
		throw new Error('A firewall change is awaiting confirmation or automatic rollback.');
	try {
		capture('nft', ['--version'], { stdio: 'pipe' });
	} catch {
		run('apt-get', ['update']);
		run('apt-get', ['install', '-y', '--no-install-recommends', 'nftables'], {
			env: { ...process.env, DEBIAN_FRONTEND: 'noninteractive' }
		});
	}
	const interfaces = [
		...new Set(
			[4, 6].flatMap((version) =>
				JSON.parse(capture('ip', ['-json', `-${version}`, 'route', 'show', 'default'])).map(
					(route) => route.dev
				)
			)
		)
	];
	const ports = [
		...new Set(
			[...capture('/usr/sbin/sshd', ['-T']).matchAll(/^port (\d+)$/gm)].map((match) =>
				Number(match[1])
			)
		)
	];
	const connectionPort = Number(process.env.SSH_CONNECTION?.split(' ')[3]);
	if (Number.isInteger(connectionPort) && connectionPort > 0 && !ports.includes(connectionPort))
		ports.push(connectionPort);
	const candidate = `${stateRoot}/firewall-candidate.nft`;
	await atomicWrite(candidate, firewallRules(manifest.cloudflare, interfaces, ports));
	run('nft', ['--check', '--file', candidate]);

	let previousRules = '';
	try {
		previousRules = capture('nft', ['list', 'table', 'inet', 'rustier_site'], { stdio: 'pipe' });
	} catch (error) {
		if (!String(error.stderr).includes('No such file or directory')) throw error;
	}
	const rollback = `${stateRoot}/firewall-rollback-${release}.nft`;
	await atomicWrite(
		rollback,
		`add table inet rustier_site\ndelete table inet rustier_site\n${previousRules}\n`
	);
	const previous = async (file) => {
		try {
			return await fs.readFile(file, 'utf8');
		} catch (error) {
			if (error.code === 'ENOENT') return null;
			throw error;
		}
	};
	let enabled = false;
	try {
		enabled =
			capture('systemctl', ['is-enabled', 'rustier-firewall.service'], { stdio: 'pipe' }) ===
			'enabled';
	} catch {
		/* No managed firewall has been installed yet. */
	}
	const timer = `rustier-firewall-rollback-${release}`;
	await atomicWrite(
		`${stateRoot}/firewall-pending.json`,
		JSON.stringify({
			release,
			rollback,
			timer,
			enabled,
			previousConfig: await previous(`${stateRoot}/firewall.nft`),
			previousUnit: await previous('/etc/systemd/system/rustier-firewall.service')
		})
	);
	try {
		run('systemd-run', [
			'--unit',
			timer,
			'--on-active=120s',
			'--timer-property=AccuracySec=1s',
			'/usr/bin/flock',
			`${stateRoot}/deploy.lock`,
			process.execPath,
			`${root}/releases/${release}/deploy.mjs`,
			'--rollback-firewall',
			release
		]);
		run('nft', ['--file', candidate]);
	} catch (error) {
		await rollbackFirewall(release);
		throw error;
	}
	console.log('Firewall applied; a new SSH connection must confirm it within two minutes.');
}

async function confirmFirewall(release) {
	if (process.getuid() !== 0) throw new Error('Firewall operations require root.');
	const pending = await readJSON(`${stateRoot}/firewall-pending.json`, null);
	if (!pending || pending.release !== release)
		throw new Error('No matching firewall change to confirm.');
	try {
		await fs.rename(`${stateRoot}/firewall-candidate.nft`, `${stateRoot}/firewall.nft`);
		await atomicWrite(
			'/etc/systemd/system/rustier-firewall.service',
			`[Unit]
Description=Rustier host and Docker ingress firewall
DefaultDependencies=no
Wants=network-pre.target
Before=network-pre.target docker.service shutdown.target
After=systemd-modules-load.service local-fs.target
Conflicts=shutdown.target

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/sbin/nft --file ${stateRoot}/firewall.nft
ExecReload=/usr/sbin/nft --file ${stateRoot}/firewall.nft

[Install]
WantedBy=multi-user.target
`,
			0o644
		);
		run('systemctl', ['daemon-reload']);
		run('systemctl', ['enable', '--now', 'rustier-firewall.service']);
		run('systemctl', ['stop', `${pending.timer}.timer`]);
		await fs.rm(`${stateRoot}/firewall-pending.json`);
		await fs.rm(pending.rollback, { force: true });
		console.log('Fresh SSH connection verified; firewall is persistent across reboots.');
	} catch (error) {
		await rollbackFirewall(release);
		throw error;
	}
}

async function deploy(release) {
	const directory = `${root}/releases/${release}`;
	await fs.chmod(directory, 0o755);
	const manifest = await readJSON(`${directory}/manifest.json`);
	if (
		manifest.release !== release ||
		!/^rustier-site-compiler:[a-f0-9]{64}$/.test(manifest.compilerImage)
	)
		throw new Error('Invalid deployment manifest.');
	const host = await hostInfo(manifest.compilerImage);
	installDocker(host);
	validateDomain(manifest.domain);
	const previous = host.state;
	validateCloudflare(manifest.cloudflare);
	const compilerChanged =
		previous?.compilerImage !== manifest.compilerImage || !host.compilerPresent;
	const configChanged =
		previous?.configHash !== manifest.configHash || previous?.domain !== manifest.domain;
	const tokenFile = `${stateRoot}/compiler-token`;
	let token;
	try {
		if ((await fs.lstat(tokenFile)).isSymbolicLink())
			throw new Error('Compiler token must not be a symlink.');
		token = (await fs.readFile(tokenFile, 'utf8')).trim();
	} catch (error) {
		if (error.code !== 'ENOENT' || previous) throw error;
		token = randomBytes(32).toString('hex');
		await atomicWrite(tokenFile, `${token}\n`);
	}
	if (!/^[a-f0-9]{64}$/.test(token)) throw new Error('Invalid stored compiler token.');
	await fs.chmod(tokenFile, 0o600);
	const apparmor = capture('docker', ['info', '--format', '{{json .SecurityOptions}}']).includes(
		'name=apparmor'
	);
	const environment = (deployment) => ({
		...process.env,
		DOCKER_HOST: 'unix:///var/run/docker.sock',
		COMPILER_TOKEN: token,
		COMPILER_IMAGE: deployment.compilerImage,
		SITE_DOMAIN: deployment.domain,
		CLOUDFLARE_TRUSTED_PROXIES: [...manifest.cloudflare.ipv4, ...manifest.cloudflare.ipv6].join(' ')
	});
	const compose = (deployment, ...args) =>
		run(
			'docker',
			[
				'compose',
				'--env-file',
				'/dev/null',
				'-p',
				'rustier-site',
				'-f',
				`${root}/config/compose.yaml`,
				'-f',
				`${root}/config/compose.production.yaml`,
				...(apparmor ? ['-f', `${root}/config/compose.apparmor.yaml`] : []),
				...args
			],
			{ env: environment(deployment) }
		);
	const activateConfig = async (deployment) => {
		await fs.mkdir(`${root}/config`, { recursive: true });
		for (const file of configFiles)
			await atomicWrite(
				`${root}/config/${file}`,
				await fs.readFile(`${root}/releases/${deployment.release}/config/${file}`),
				0o644
			);
	};
	const loadProfile = () => {
		if (!apparmor) return;
		try {
			capture('apparmor_parser', ['--version'], { stdio: 'pipe' });
		} catch {
			run('apt-get', ['update']);
			run('apt-get', ['install', '-y', 'apparmor'], {
				env: { ...process.env, DEBIAN_FRONTEND: 'noninteractive' }
			});
		}
		run('apparmor_parser', [
			'--skip-kernel-load',
			'--skip-cache',
			`${root}/config/compiler.apparmor`
		]);
		run('install', [
			'-m',
			'644',
			`${root}/config/compiler.apparmor`,
			'/etc/apparmor.d/site-compiler-service'
		]);
		run('apparmor_parser', ['-r', '/etc/apparmor.d/site-compiler-service']);
	};
	const healthy = async (service) => {
		for (let attempt = 0; attempt < 120; attempt++) {
			const id = capture(
				'docker',
				[
					'compose',
					'-p',
					'rustier-site',
					'-f',
					`${root}/config/compose.yaml`,
					'-f',
					`${root}/config/compose.production.yaml`,
					'ps',
					'-q',
					service
				],
				{ env: environment(manifest) }
			);
			if (!id) throw new Error(`${service} is not running.`);
			const status = capture('docker', ['inspect', '--format', '{{.State.Health.Status}}', id]);
			if (status === 'healthy') return;
			if (status === 'unhealthy') throw new Error(`${service} is unhealthy.`);
			await new Promise((resolve) => setTimeout(resolve, 1000));
		}
		throw new Error(`${service} health check timed out.`);
	};
	if (!host.compilerPresent) {
		run('docker', ['load', '-i', `${directory}/compiler.tar`]);
		await fs.rm(`${directory}/compiler.tar`);
	}
	let activated = false;
	try {
		activated = true;
		if (configChanged) await activateConfig(manifest);
		compose(manifest, 'config', '--quiet');
		if (configChanged || compilerChanged) loadProfile();
		if (compilerChanged) {
			compose(
				manifest,
				'up',
				'-d',
				'--no-deps',
				'--no-build',
				'--pull',
				'never',
				'--wait',
				'--wait-timeout',
				'120',
				'compiler'
			);
			compose(
				manifest,
				'exec',
				'-T',
				'compiler',
				'node',
				'--input-type=module',
				'-e',
				`
const response = await fetch('http://127.0.0.1:3001/build', {
 method: 'POST', headers: { authorization: 'Bearer ' + process.env.COMPILER_TOKEN, 'content-type': 'application/json' },
 body: JSON.stringify({ sessionId: crypto.randomUUID(), code: 'pub fn deployment_probe() -> u32 { 42 }', modules: [] }),
 signal: AbortSignal.timeout(80000)
});
const result = await response.json();
if (!response.ok || result.type !== 'ok' || !WebAssembly.validate(Buffer.from(result.wasm, 'base64')))
 throw new Error('Sandbox compilation probe failed');
`
			);
		} else {
			await healthy('compiler');
		}
		await switchRelease(release);
		if (configChanged) {
			compose(
				manifest,
				'up',
				'-d',
				'--no-deps',
				'--no-build',
				'--pull',
				'missing',
				'--force-recreate',
				'--wait',
				'--wait-timeout',
				'90',
				'site'
			);
			compose(
				manifest,
				'up',
				'-d',
				'--no-deps',
				'--no-build',
				'--pull',
				'missing',
				'--force-recreate',
				'--wait',
				'--wait-timeout',
				'90',
				'caddy'
			);
		} else {
			compose(manifest, 'restart', 'site');
			await healthy('site');
		}
		run('curl', [
			'--fail',
			'--silent',
			'--show-error',
			'--retry',
			'10',
			'--retry-all-errors',
			'--retry-delay',
			'2',
			'--retry-max-time',
			'60',
			'--connect-timeout',
			'5',
			'--max-time',
			'10',
			'--output',
			'/dev/null',
			`https://${manifest.domain}/playground`
		]);
		await fs.writeFile(`${directory}/.deployed`, '');
		await atomicWrite(`${stateRoot}/deployment.json`, JSON.stringify(manifest));
		activated = false;
		console.log(
			`Deployed ${release} at https://${manifest.domain}; compiler ${compilerChanged ? 'updated' : 'unchanged'}.`
		);
	} catch (error) {
		if (activated) {
			try {
				const logs = capture(
					'docker',
					[
						'compose',
						'--env-file',
						'/dev/null',
						'-p',
						'rustier-site',
						'-f',
						`${root}/config/compose.yaml`,
						'-f',
						`${root}/config/compose.production.yaml`,
						'logs',
						'--no-color',
						'--tail',
						'100'
					],
					{ env: environment(manifest) }
				);
				await atomicWrite(`${stateRoot}/last-failure.log`, logs);
				console.error(logs);
			} catch (logError) {
				console.error(`Could not save deployment logs: ${logError.message}`);
			}
			console.error('Deployment failed; restoring the previous release.');
			try {
				if (previous) {
					await activateConfig(previous);
					await switchRelease(previous.release);
					if (compilerChanged) {
						loadProfile();
						compose(
							previous,
							'up',
							'-d',
							'--no-deps',
							'--no-build',
							'--pull',
							'never',
							'--wait',
							'--wait-timeout',
							'120',
							'compiler'
						);
					}
					if (configChanged) {
						compose(
							previous,
							'up',
							'-d',
							'--no-deps',
							'--no-build',
							'--pull',
							'never',
							'--force-recreate',
							'--wait',
							'--wait-timeout',
							'90',
							'site',
							'caddy'
						);
					} else {
						compose(previous, 'restart', 'site');
					}
					await healthy('site');
				} else {
					compose(manifest, 'down', '--remove-orphans');
				}
			} catch (rollbackError) {
				console.error('Rollback failed:', rollbackError.message);
			}
		}
		throw error;
	}
	for (const entry of await fs.readdir(`${root}/releases`, { withFileTypes: true })) {
		if (
			entry.isDirectory() &&
			/^\d+-[a-f0-9]{12}$/.test(entry.name) &&
			entry.name !== release &&
			entry.name !== previous?.release
		) {
			const candidate = `${root}/releases/${entry.name}`;
			try {
				await fs.access(`${candidate}/.deployed`);
				await fs.rm(candidate, { recursive: true, force: true });
			} catch (error) {
				if (error.code !== 'ENOENT') console.warn(`Old release cleanup failed: ${error.message}`);
			}
		}
	}
}

try {
	const [mode, argument] = process.argv.slice(2);
	if (mode === '--help' || mode === '-h' || !mode) {
		console.log(`Usage: node packages/site/scripts/deploy.mjs SSH_HOST [DOMAIN]

DOMAIN defaults to rustier.dev.

Requires local Node 24+, pnpm install, the local Rustier/Rust build setup, rsync,
and trusted root SSH access. Docker with Buildx is needed only for compiler updates.
Host: Ubuntu 24.04+, ARM64/AMD64, DNS pointing here and TCP 80/443 open.
The hostname must be proxied through Cloudflare. Each deployment refreshes the
official Cloudflare ranges, enforces an IPv4/IPv6 default-deny ingress firewall
(including Docker), and permits web traffic only from Cloudflare. SSH ports,
established connections, ICMP and DHCP remain allowed. A timed rollback protects
SSH access until a fresh connection confirms the rules. Firewall rules persist
across reboots; this script manages only its own nftables table.
Builds run locally for the server architecture. Only built artifacts are uploaded.
Unchanged compiler inputs reuse the existing image and workers. Site-only updates
switch the built files and restart Node; Caddy and the compiler stay running.
First deployment installs Node (deployment agent), Docker and HTTPS automatically.
A site restart briefly interrupts requests; this is not a zero-downtime deployment.
The previous release is kept for automatic rollback. Images are never globally pruned.
Back up /etc/rustier-site and the Caddy certificate volumes.`);
	} else if (mode === '--host-info') {
		delete process.env.DOCKER_CONTEXT;
		delete process.env.DOCKER_TLS_VERIFY;
		delete process.env.DOCKER_CERT_PATH;
		process.env.DOCKER_HOST = 'unix:///var/run/docker.sock';
		console.log(JSON.stringify(await hostInfo(argument)));
	} else if (
		['--prepare', '--remote', '--firewall', '--confirm-firewall', '--rollback-firewall'].includes(
			mode
		)
	) {
		if (!/^\d+-[a-f0-9]{12}$/.test(argument ?? '')) throw new Error('Invalid release ID.');
		delete process.env.DOCKER_CONTEXT;
		delete process.env.DOCKER_TLS_VERIFY;
		delete process.env.DOCKER_CERT_PATH;
		process.env.DOCKER_HOST = 'unix:///var/run/docker.sock';
		if (mode === '--prepare') await prepare(argument);
		else if (mode === '--firewall') await enforceFirewall(argument);
		else if (mode === '--confirm-firewall') await confirmFirewall(argument);
		else if (mode === '--rollback-firewall') await rollbackFirewall(argument);
		else await deploy(argument);
	} else {
		await localDeploy(mode, argument);
	}
} catch (error) {
	console.error(`deploy: ${error.message}`);
	process.exitCode = 1;
}
