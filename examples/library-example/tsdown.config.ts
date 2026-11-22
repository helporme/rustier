import { defineConfig } from "tsdown";
import rustier from "rustier/tsdown";

export default defineConfig({
  plugins: [
    rustier(),
  ],
  dts: true,
  sourcemap: true,
  exports: true,
});
