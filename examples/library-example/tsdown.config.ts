import { defineConfig } from "tsdown";
import rustier from "rustier-next";

export default defineConfig({
  plugins: [
    rustier({
      backend: { type: "node" },
    }),
  ],
  dts: true,
  sourcemap: true,
  exports: true,
});
