import { spawn } from 'node:child_process';
import * as fs from 'node:fs/promises';
import http from 'node:http';
import path from 'node:path';

const root = process.env.DEPLOY_ROOT ?? '/deploy';
const controlSocket = process.env.DEPLOY_CONTROL_SOCKET ?? '/tmp/rustier-deploy.sock';

function request(socketPath, options = {}) {
	return new Promise((resolve, reject) => {
		const req = http.request({ socketPath, path: '/playground', ...options }, (res) => {
			res.resume();
			res.once('end', () => resolve(res));
		});
		req.once('error', reject);
		req.setTimeout(30000, () => req.destroy(new Error('Site request timed out.')));
		req.end();
	});
}

if (process.argv[2] === 'activate') {
	const res = await request(controlSocket, { method: 'POST', path: `/${process.argv[3]}` });
	if (res.statusCode !== 200)
		throw new Error(`Site activation failed (HTTP ${res.statusCode}); see container logs.`);
	console.log(`Site activated: ${process.argv[3]}`);
} else {
	let active;
	let activating = false;
	const children = new Set();

	async function launch(release) {
		if (!/^\d+-[a-f0-9]{12}$/.test(release)) throw new Error('Invalid release ID.');
		if (active?.release === release) return;
		const socket = `/tmp/rustier-${release}.sock`;
		await fs.rm(socket, { force: true });
		const directory = path.join(root, 'releases', release, 'site');
		const child = spawn(process.execPath, ['build/index.js'], {
			cwd: directory,
			stdio: 'inherit',
			env: { ...process.env, PORT: undefined, SOCKET_PATH: socket }
		});
		const candidate = { release, child, socket, pending: 0 };
		children.add(child);
		let failure;
		child.once('error', (error) => {
			failure = error;
		});
		child.once('exit', (code) => {
			children.delete(child);
			failure = new Error(`Site process exited (${code}).`);
			if (active === candidate) {
				console.error(failure.message);
				process.exit(1); // Docker restarts the supervisor from the committed release.
			}
		});
		try {
			const deadline = Date.now() + 30000;
			for (;;) {
				if (failure) throw failure;
				try {
					if ((await request(socket)).statusCode === 200) break;
				} catch {
					/* The child has not bound its socket yet. */
				}
				if (Date.now() > deadline) throw new Error('New site release did not become ready.');
				await new Promise((resolve) => setTimeout(resolve, 100));
			}
			const previous = active;
			active = candidate;
			if (previous) {
				if (!previous.pending) previous.child.kill('SIGTERM');
				else setTimeout(() => previous.child.kill('SIGTERM'), 180000).unref();
			}
		} catch (error) {
			child.kill('SIGTERM');
			throw error;
		}
	}

	const server = http.createServer((incoming, outgoing) => {
		const current = active;
		if (!current) {
			outgoing.writeHead(503).end('Site is starting.');
			return;
		}
		current.pending++;
		const upstream = http.request(
			{
				socketPath: current.socket,
				path: incoming.url,
				method: incoming.method,
				headers: incoming.headers
			},
			(res) => {
				outgoing.writeHead(res.statusCode, {
					...res.headers,
					'x-rustier-release': current.release
				});
				res.pipe(outgoing);
				res.once('error', () => outgoing.destroy());
			}
		);
		upstream.once('error', () => {
			if (!outgoing.headersSent) outgoing.writeHead(502).end('Site process unavailable.');
			else outgoing.destroy();
		});
		outgoing.once('close', () => {
			upstream.destroy();
			current.pending--;
			if (current !== active && !current.pending) current.child.kill('SIGTERM');
		});
		incoming.once('aborted', () => upstream.destroy());
		incoming.pipe(upstream);
	});

	await launch(await fs.readlink(path.join(root, 'releases/current')));
	await fs.rm(controlSocket, { force: true });
	const control = http.createServer(async (req, res) => {
		const release = req.url.slice(1);
		if (req.method !== 'POST' || !/^\d+-[a-f0-9]{12}$/.test(release)) {
			res.writeHead(400).end();
			return;
		}
		if (activating) {
			res.writeHead(409).end();
			return;
		}
		activating = true;
		try {
			await launch(release);
			res.writeHead(200).end();
		} catch (error) {
			console.error(`Activation rejected: ${error.message}`);
			res.writeHead(503).end();
		} finally {
			activating = false;
		}
	});
	control.listen(controlSocket, () => {
		void fs.chmod(controlSocket, 0o600);
	});
	server.listen(Number(process.env.PORT ?? 3000), process.env.HOST ?? '::');
	for (const signal of ['SIGTERM', 'SIGINT'])
		process.once(signal, () => {
			control.close();
			server.close(() => {
				active = undefined;
				for (const child of children) child.kill(signal);
				process.exit(0);
			});
		});
}
