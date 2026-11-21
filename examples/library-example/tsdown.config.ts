import { defineConfig } from "tsdown";
import rustier from "rustier-next";

export default defineConfig({
  plugins: [
    rustier({
      targets: [
        'aarch64-apple-darwin',
        { target: 'wasm32-unknown-unknown', loading: 'module' },
      ],
    }),
  ],
  dts: true,
  sourcemap: true,
  exports: true,
});
