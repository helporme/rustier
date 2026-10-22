import { onMount } from 'svelte';
import { isSharedSource, MAX_SHARE_URL_LENGTH, type SharedSnippet } from './share';

const STORAGE_KEY = 'rustier.shared-snippets.v1';

function isSharedSnippet(value: unknown): value is SharedSnippet {
	return (
		typeof value === 'object' &&
		value !== null &&
		'code' in value &&
		typeof value.code === 'string' &&
		value.code.length <= MAX_SHARE_URL_LENGTH &&
		/^v1\.[A-Za-z0-9_-]+$/.test(value.code) &&
		'source' in value &&
		isSharedSource(value.source)
	);
}

export class SavedSnippets {
	entries = $state<SharedSnippet[]>([]);
	error = $state('');
	#ready = false;

	constructor() {
		onMount(() => {
			try {
				const stored: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]');
				if (!Array.isArray(stored) || !stored.every(isSharedSnippet)) {
					throw new Error('Invalid saved snippets.');
				}
				for (const entry of stored) {
					if (!this.entries.some((current) => current.code === entry.code)) {
						this.entries.push(entry);
					}
				}
				this.#ready = true;
				this.persist();
			} catch {
				this.#ready = true;
				this.error = 'Saved snippets could not be loaded. New links may not be saved.';
			}
		});
	}

	remember(snippet: SharedSnippet) {
		if (!this.entries.some((entry) => entry.code === snippet.code)) {
			this.entries.unshift(snippet);
		}
		return this.persist();
	}

	forget(code: string) {
		this.entries = this.entries.filter((entry) => entry.code !== code);
		this.persist();
	}

	private persist() {
		if (!this.#ready) return true;
		try {
			localStorage.setItem(STORAGE_KEY, JSON.stringify(this.entries));
			this.error = '';
			return true;
		} catch {
			this.error = 'Changes could not be saved. Local storage may be full or unavailable.';
			return false;
		}
	}
}
