import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    index: "src/index.ts",
  },
  platform: "node",
  format: "esm",
  target: "node24",
  sourcemap: true,
  outputOptions: {
    codeSplitting: false,
  },
  deps: {
    alwaysBundle: ["valibot"],
    neverBundle: [
      "fastify",
      "rustier",
      "rustier-native",
      "rolldown",
      /\.node$/,
    ],
  },
});
