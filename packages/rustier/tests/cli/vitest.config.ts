import { defineConfig } from 'vitest/config'

export default defineConfig(({ mode }) => ({
  root: import.meta.dirname,
  test: {
    include: ['**/*.test.ts'],
    provide: {
      regenerateCliSnapshots: mode === 'regen',
    },
  },
}))
