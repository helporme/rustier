import { cp, mkdir, readFile, symlink } from "node:fs/promises";
import { createInterface } from "node:readline";
import * as v from "valibot";
import { buildInputSchema } from "../../../site/src/lib/playground/compilation-schema.ts";
import { compile } from "./compile.ts";

await mkdir("/work/cargo", { recursive: true });
await symlink("/opt/cargo/registry", "/work/cargo/registry");
await cp("/opt/playground", "/work/project", {
  recursive: true,
  dereference: false,
});

const warmed = await compile({
  code: await readFile("/work/project/src/lib.rs", "utf8"),
  modules: [],
});

if (warmed.type !== "ok") throw new Error(warmed.message);

process.stdout.write('{"type":"ready"}\n');

for await (const line of createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
})) {
  try {
    const input = v.parse(buildInputSchema, JSON.parse(line));
    const result = await compile(input);

    process.stdout.write(`${JSON.stringify(result)}\n`);
  } catch (cause) {
    process.stderr.write(
      cause instanceof Error ? (cause.stack ?? cause.message) : String(cause),
    );
    process.exit(1);
  }
}
