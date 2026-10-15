import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
	COMPILER_MAX_PENDING_PER_IP: {
		schema: (value) => {
			const limit = Number(value ?? 1);

			if (!Number.isInteger(limit) || limit < 1 || limit > 8)
				throw new Error('COMPILER_MAX_PENDING_PER_IP must be between 1 and 8');

			return limit;
		}
	},
	COMPILER_URL: {
		schema: (value) => {
			if (!value) return undefined;

			const url = new URL(value);

			if (url.protocol !== 'http:' && url.protocol !== 'https:')
				throw new Error('COMPILER_URL must be an HTTP URL');

			return value;
		}
	},
	COMPILER_TOKEN: {
		schema: (value) => {
			if (!value) return undefined;
			if (value.length < 32) throw new Error('COMPILER_TOKEN must contain at least 32 characters');

			return value;
		}
	}
});
