import { defineConfig } from 'tsdown'

export default defineConfig({
  entry: {
    index: 'src/index.ts',
    tsdown: 'src/tsdown.ts',
  },
  dts: true,
  deps: {
    neverBundle: ['server-next', 'rolldown'],
  },
})
