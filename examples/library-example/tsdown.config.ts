import { defineConfig } from "tsdown";
import rustCurse from "rust-curse/tsdown";

export default defineConfig({
  plugins: [rustCurse({ platform: "wasm" })],
  dts: true,
  exports: true,
  // ...config options
});
