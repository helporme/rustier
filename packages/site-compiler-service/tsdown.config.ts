import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    server: "src/server.ts",
    worker: "src/compilation/worker.ts",
    seed: "src/compilation/seed.ts",
  },
  platform: "node",
  format: "esm",
  target: "node24",
  sourcemap: true,
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
