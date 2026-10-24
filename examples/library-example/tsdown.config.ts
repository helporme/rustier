import { defineConfig } from "tsdown";
import rustier from "rustier/tsdown";

export default defineConfig({
  plugins: [
    rustier({
      crates: { "library-example": { path: "." } },
      platform: { type: "wasm", loading: "node-js" },
    }),
  ],
  dts: true,
  exports: true,
});
