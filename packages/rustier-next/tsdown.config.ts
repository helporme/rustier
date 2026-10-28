import { defineConfig } from 'tsdown'

export default defineConfig({
  entry: {
    index: 'src/index.ts',
    rolldown: 'src/rolldown.ts',
    tsdown: 'src/tsdown.ts',
    vite: 'src/vite.ts',
  },
  dts: true,
  deps: {
    neverBundle: ['server-next', 'rolldown', 'vite'],
  },
})
