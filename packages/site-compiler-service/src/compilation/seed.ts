import { mkdir, symlink } from "node:fs/promises";
import path from "node:path";
import { compile } from "./compile.ts";

const root = process.env.PLAYGROUND_ROOT!;
const tools = path.join(root, ".rustier/tools/wasm_bindgen/bin");

await mkdir(tools, { recursive: true });
await symlink("/opt/cargo/bin/wasm-bindgen", path.join(tools, "wasm-bindgen"));

const result = await compile({
  code: 'pub fn hello() -> String { "Hello, world!".to_owned() }',
});

if (result.type !== "ok") throw new Error(result.message);

process.stdout.write(`Prepared playground (${result.duration.toFixed(0)}ms)\n`);
