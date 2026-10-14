import { highlight, tokenColors } from '#lib/ui/highlight';
import type { TokenizationRequest, TokenizationResponse } from './tokenization';

self.onmessage = ({ data }: MessageEvent<TokenizationRequest>) => {
	try {
		const lines = highlight(data.code, data.language);
		const encoded: number[] = [];
		let previousLine = 0;
		let previousColumn = 0;

		for (const [line, tokens] of lines.entries()) {
			let column = 0;

			for (const token of tokens) {
				if (!token.content.length) continue;

				const color = tokenColors.findIndex((name) => token.color === `var(--color-${name})`);

				encoded.push(
					line - previousLine,
					line === previousLine ? column - previousColumn : column,
					token.content.length,
					Math.max(color, 0),
					token.fontStyle ?? 0
				);
				previousLine = line;
				previousColumn = column;
				column += token.content.length;
			}
		}

		const tokens = new Uint32Array(encoded);

		self.postMessage({ id: data.id, tokens } satisfies TokenizationResponse, {
			transfer: [tokens.buffer]
		});
	} catch (cause) {
		self.postMessage({
			id: data.id,
			error: cause instanceof Error ? cause.message : String(cause)
		} satisfies TokenizationResponse);
	}
};
