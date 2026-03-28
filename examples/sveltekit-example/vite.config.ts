import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import rustCurse from "rust-curse/vite";

export default defineConfig({
  plugins: [sveltekit(), rustCurse()],
});
