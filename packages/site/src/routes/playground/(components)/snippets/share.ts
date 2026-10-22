export const MAX_SHARE_URL_LENGTH = 8_000;
const MAX_SOURCE_BYTES = 256 * 1024;
export const MAX_TITLE_LENGTH = 100;

export type SharedSource = {
	title: string;
	rust: string;
	typescript: string;
};

export type SharedSnippet = {
	code: string;
	source: SharedSource;
};

export function sharedId(code: string) {
	return `shared:${code}`;
}

export function slugify(title: string) {
	return (
		title
			.normalize('NFKD')
			.replace(/\p{M}/gu, '')
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, '-')
			.replace(/^-|-$/g, '') || 'shared-snippet'
	);
}

export function isSharedSource(source: unknown): source is SharedSource {
	return (
		typeof source === 'object' &&
		source !== null &&
		'title' in source &&
		typeof source.title === 'string' &&
		source.title.trim().length > 0 &&
		source.title.length <= MAX_TITLE_LENGTH &&
		'rust' in source &&
		typeof source.rust === 'string' &&
		'typescript' in source &&
		typeof source.typescript === 'string' &&
		new TextEncoder().encode(JSON.stringify(source)).length <= MAX_SOURCE_BYTES
	);
}

async function readLimited(stream: ReadableStream<Uint8Array>, limit: number) {
	const reader = stream.getReader();
	const chunks: Uint8Array[] = [];
	let length = 0;
	try {
		while (true) {
			const { done, value } = await reader.read();
			if (done) break;
			length += value.length;
			if (length > limit) throw new Error('Shared code is too large.');
			chunks.push(value);
		}
	} finally {
		await reader.cancel();
		reader.releaseLock();
	}
	const bytes = new Uint8Array(length);
	let offset = 0;
	for (const chunk of chunks) {
		bytes.set(chunk, offset);
		offset += chunk.length;
	}
	return bytes;
}

export async function createShareUrl(playgroundUrl: URL, source: SharedSource) {
	const bytes = new TextEncoder().encode(JSON.stringify(source));
	if (bytes.length > MAX_SOURCE_BYTES) throw new Error('Shared code is too large.');
	if (!isSharedSource(source)) throw new Error('Enter a title of up to 100 characters.');
	const compressed = await readLimited(
		new Blob([bytes]).stream().pipeThrough(new CompressionStream('gzip')),
		MAX_SHARE_URL_LENGTH
	);
	const encoded = btoa(String.fromCharCode(...compressed))
		.replaceAll('+', '-')
		.replaceAll('/', '_')
		.replace(/=+$/, '');
	const url = new URL(playgroundUrl);
	url.pathname = `${url.pathname.replace(/\/$/, '')}/${slugify(source.title)}`;
	url.searchParams.set('code', `v1.${encoded}`);
	if (url.href.length > MAX_SHARE_URL_LENGTH) {
		throw new Error('Share link exceeds the 8 KB limit. Try shortening the code.');
	}
	return url.href;
}

export async function decodeSharedSource(code: string): Promise<SharedSource> {
	if (code.length > MAX_SHARE_URL_LENGTH || !/^v1\.[A-Za-z0-9_-]+$/.test(code)) {
		throw new Error('Invalid share link.');
	}
	const bytes = Uint8Array.from(
		atob(code.slice(3).replaceAll('-', '+').replaceAll('_', '/')),
		(character) => character.charCodeAt(0)
	);
	const decoded = await readLimited(
		new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip')),
		MAX_SOURCE_BYTES
	);
	const source = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(decoded));
	// Links produced before titles were introduced still contain an example ID.
	if (source && source.title === undefined && typeof source.example === 'string') {
		source.title = 'Shared snippet';
	}
	if (!isSharedSource(source)) {
		throw new Error('Invalid shared code.');
	}
	return { title: source.title, rust: source.rust, typescript: source.typescript };
}
