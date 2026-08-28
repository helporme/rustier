import { defineConfig } from "tsdown";
import rustCurse from "rust-curse/tsdown";

export default defineConfig({
  plugins: [rustCurse({ target: "node" })],
  dts: true,
  exports: true,
});
