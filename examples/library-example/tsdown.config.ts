import { defineConfig } from "tsdown";
import rustier from "rustier/tsdown";

export default defineConfig({
  plugins: [rustier({ target: "node" })],
  dts: true,
  exports: true,
});
