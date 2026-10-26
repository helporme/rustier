import { defineConfig } from "tsdown";
import rustier from "rustier/tsdown";

export default defineConfig({
  plugins: [
    rustier({
      platform: { type: "wasm", loading: "node-js" },
    }),
  ],
  dts: true,
  exports: true,
});
