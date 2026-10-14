import type * as Monaco from 'monaco-editor/editor';
import { tokenColors } from '#lib/ui/highlight';
import TokenizationWorker from './tokenization.worker?worker';

export const tokenModifiers = ['italic', 'bold', 'underline', 'strikethrough'];

export type TokenizationLanguage = 'rust' | 'typescript' | 'shellscript';

export interface TokenizationRequest {
	id: number;
	code: string;
	language: TokenizationLanguage;
}

export type TokenizationResponse =
	| { id: number; tokens: Uint32Array }
	| { id: number; error: string };

class TokenState implements Monaco.languages.IState {
	clone() {
		return this;
	}

	equals(other: Monaco.languages.IState) {
		return other instanceof TokenState;
	}
}

export function registerTokenization(monaco: typeof Monaco) {
	let worker: Worker | undefined;
	let sequence = 0;
	const cache = new WeakMap<
		Monaco.editor.ITextModel,
		{
			version: number;
			language: TokenizationLanguage;
			tokens: Monaco.languages.SemanticTokens;
		}
	>();
	const pending = new Map<
		number,
		{ resolve: (tokens: Monaco.languages.SemanticTokens | null) => void; dispose: () => void }
	>();
	const legend = {
		tokenTypes: tokenColors.map((color) => `rustier-${color}`),
		tokenModifiers
	};

	for (const language of ['rust', 'typescript', 'shellscript'] satisfies TokenizationLanguage[]) {
		monaco.languages.setTokensProvider(language, {
			getInitialState: () => new TokenState(),
			tokenize: (_line, state) => ({ tokens: [{ startIndex: 0, scopes: '' }], endState: state })
		});
		monaco.languages.registerDocumentSemanticTokensProvider(language, {
			getLegend: () => legend,
			releaseDocumentSemanticTokens() {},
			provideDocumentSemanticTokens(model, _lastResultId, cancellation) {
				if (cancellation.isCancellationRequested) return null;

				const version = model.getVersionId();
				const cached = cache.get(model);
				if (cached?.version === version && cached.language === language) return cached.tokens;

				if (!worker) {
					worker = new TokenizationWorker({ name: 'rustier-tokenization' });
					worker.onmessage = ({ data }: MessageEvent<TokenizationResponse>) => {
						const request = pending.get(data.id);
						if (!request) return;

						pending.delete(data.id);
						request.dispose();

						if ('error' in data) {
							console.error('Syntax tokenization failed:', data.error);
							request.resolve(null);
						} else {
							request.resolve({ data: data.tokens });
						}
					};
					worker.onerror = (event) => {
						console.error('Syntax tokenization worker failed:', event.message);

						for (const request of pending.values()) {
							request.dispose();
							request.resolve(null);
						}

						pending.clear();
						worker?.terminate();
						worker = undefined;
					};
				}

				const id = ++sequence;

				return new Promise<Monaco.languages.SemanticTokens | null>((resolve) => {
					const listener = cancellation.onCancellationRequested(() => {
						pending.delete(id);
						listener.dispose();
						resolve(null);
					});
					pending.set(id, {
						dispose: () => listener.dispose(),
						resolve: (tokens) => {
							if (model.isDisposed() || model.getVersionId() !== version) {
								resolve(null);
								return;
							}

							if (tokens) cache.set(model, { version, language, tokens });
							resolve(tokens);
						}
					});
					worker!.postMessage({
						id,
						code: model.getValue(),
						language
					} satisfies TokenizationRequest);
				});
			}
		});
	}
}
