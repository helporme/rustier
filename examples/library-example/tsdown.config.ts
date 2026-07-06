import { defineConfig } from "tsdown";
import rustCurse from "rust-curse/tsdown";
import { TupleStruct, X } from "./declare";


export default defineConfig({
  plugins: [rustCurse({ platform: "node" })],
  dts: true,
  exports: true,
  // ...config options
});
