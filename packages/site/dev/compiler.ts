import { spawn, type ChildProcess } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import path from 'node:path';
import { loadEnv, type Plugin } from 'vite';

type ManagedCompiler = {
	users: number;
	ready: Promise<void>;
	dispose: () => Promise<void>;
};

// Vite creates the replacement server before closing the old one on config reload.
const key = Symbol.for('rustier.site.dev.compilers');
const globals = globalThis as typeof globalThis & {
	[key: symbol]: Map<string, ManagedCompiler>;
};
const compilers = (globals[key] ??= new Map());

/** Starts a private compiler for Vite dev; external compiler configurations are left alone. */
export function compilerService(): Plugin {
	let stop: (() => Promise<void>) | undefined;

	const attach = (compiler: ManagedCompiler) => {
		compiler.users++;
		let released = false;
		stop = async () => {
			if (released) return;

			released = true;
			if (--compiler.users === 0) await compiler.dispose();
		};
	};

	return {
		name: 'site-compiler-service',
		apply: 'serve',
		enforce: 'pre',
		configureServer: {
			order: 'pre',
			async handler(server) {
				const current = compilers.get(server.config.root);

				if (current) {
					attach(current);
					await current.ready;
					return;
				}

				const configured = loadEnv(server.config.mode, server.config.envDir, '');

				if (configured.COMPILER_URL || configured.COMPILER_TOKEN) {
					if (!configured.COMPILER_URL || !configured.COMPILER_TOKEN)
						throw new Error(
							'Set both COMPILER_URL and COMPILER_TOKEN for an external compiler, or unset both for automatic development startup.'
						);

					return;
				}

				const token = randomBytes(32).toString('hex');
				const project = `rustier-site-dev-${process.pid}-${randomBytes(4).toString('hex')}`;
				const args = [
					'compose',
					'--project-directory',
					server.config.root,
					'-p',
					project,
					'-f',
					path.join(server.config.root, 'compose.yaml'),
					'-f',
					path.join(server.config.root, 'compose.dev.yaml')
				];
				const environment = {
					...process.env,
					...configured,
					COMPILER_TOKEN: token,
					COMPILER_PORT: '0'
				};
				const previous = {
					COMPILER_TOKEN: process.env.COMPILER_TOKEN,
					COMPILER_URL: process.env.COMPILER_URL
				};
				let url: string | undefined;
				let active: ChildProcess | undefined;
				let operation: Promise<string> | undefined;
				let stopping: Promise<void> | undefined;

				const docker = (command: string[], capture = false): Promise<string> =>
					new Promise((resolve, reject) => {
						const child = spawn('docker', command, {
							cwd: server.config.root,
							env: environment,
							stdio: capture ? ['ignore', 'pipe', 'pipe'] : 'inherit'
						});
						active = child;
						let output = '';
						let errors = '';

						child.stdout?.on('data', (chunk: Buffer) => (output += chunk.toString()));
						child.stderr?.on('data', (chunk: Buffer) => (errors += chunk.toString()));
						child.once('error', reject);
						child.once('close', (code) => {
							if (active === child) active = undefined;
							if (code === 0) resolve(output.trim());
							else
								reject(
									new Error(
										`Docker exited with ${code ?? 'a signal'}${errors ? `: ${errors}` : ''}`
									)
								);
						});
					});

				const securityOptions = await docker(
					['info', '--format', '{{json .SecurityOptions}}'],
					true
				).catch((cause) => {
					throw new Error('Cannot start the development compiler. Is Docker running?', { cause });
				});

				if ((JSON.parse(securityOptions) as string[]).includes('name=apparmor'))
					args.push('-f', path.join(server.config.root, 'compose.apparmor.yaml'));

				const interrupt = () => {
					void Promise.all([...compilers.values()].map((compiler) => compiler.dispose())).finally(
						() => process.exit(0)
					);
				};

				const dispose = () =>
					(stopping ??= (async () => {
						active?.kill('SIGTERM');
						await operation?.catch(() => undefined);

						try {
							await docker([...args, 'down', '--remove-orphans']);
						} catch (cause) {
							server.config.logger.warn(
								`Could not remove development compiler ${project}: ${cause}`
							);
						} finally {
							for (const [key, generated] of [
								['COMPILER_TOKEN', token],
								['COMPILER_URL', url]
							] as const) {
								if (process.env[key] !== generated) continue;
								if (previous[key] === undefined) delete process.env[key];
								else process.env[key] = previous[key];
							}

							process.off('SIGINT', interrupt);
							process.off('SIGTERM', interrupt);
							compilers.delete(server.config.root);
						}
					})());

				process.once('SIGINT', interrupt);
				process.once('SIGTERM', interrupt);
				const compiler: ManagedCompiler = { users: 0, ready: Promise.resolve(), dispose };
				compilers.set(server.config.root, compiler);
				attach(compiler);
				server.config.logger.info(
					'Starting the development compiler (the first image build may take a few minutes)…'
				);

				compiler.ready = (async () => {
					try {
						operation = docker([
							...args,
							'up',
							'--build',
							'--detach',
							'--wait',
							'--wait-timeout',
							'90',
							'compiler'
						]);
						await operation;
						if (stopping) throw new Error('Development compiler startup cancelled');

						operation = docker([...args, 'port', 'compiler', '3001'], true);
						const address = await operation;
						if (stopping) throw new Error('Development compiler startup cancelled');

						if (!/^127\.0\.0\.1:\d+$/.test(address))
							throw new Error(`Unexpected development compiler address: ${address}`);

						url = `http://${address}`;
						process.env.COMPILER_TOKEN = token;
						process.env.COMPILER_URL = url;
						server.config.logger.info(`Development compiler ready at ${url}`);
					} catch (cause) {
						await dispose();
						throw cause;
					}
				})();
				await compiler.ready;
			}
		},
		async closeBundle() {
			await stop?.();
		}
	};
}
