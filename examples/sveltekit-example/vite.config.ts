import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import rustCurse from "rust-curse/vite";

export default defineConfig({
  plugins: [
    sveltekit(),
    rustCurse({
      sources: {
        include: ["./src/**/*.js", "./src/**/*.ts", "./src/**/*.svelte"],
        exclude: [
          // ./src/lib/server folder
          // "./src/lib/server/**/*.js",
          // "./src/lib/server/**/*.ts",
          // "./src/lib/server/**/*.svelte",
          // .server suffix
          // "./src/**/*.server.js",
          // "./src/**/*.server.ts",
          // "./src/**/*.server.svelte",
          // .remote suffix
          // "./src/**/*.remote.js",
          // "./src/**/*.remote.ts",
        ],
      },
      ssr: {
        sources: ["./src/**/*.js", "./src/**/*.ts", "./src/**/*.svelte"],
      },
    }),
  ],
});
