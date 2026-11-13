import { mkdir, readFile } from "node:fs/promises";
import path from "node:path";
import { rolldown, RolldownMagicString } from "rolldown";
import { Server } from "rustier-server";
import { EXECUTION_CONSOLE } from "../../../site/src/lib/playground/compilation/execution-protocol.ts";
import type {
  BuildInput,
  CompilerResult,
} from "../../../site/src/lib/playground/compilation/schema.ts";

const root = path.resolve(process.env.PLAYGROUND_ROOT ?? "/work/project");
const entryId = path.join(root, "index.ts");
const runtimeId = "playground:runtime";
const wasmModuleId = "playground:wasm";
const profile = "playground";
let server: Server | undefined;

export async function compile({
  typescript,
}: BuildInput): Promise<CompilerResult> {
  const started = performance.now();
  let bundle: Awaited<ReturnType<typeof rolldown>> | undefined;

  try {
    await mkdir(root, { recursive: true });
    server ??= new Server({
      rootDir: root,
      profiles: {
        [profile]: {
          backend: { type: "wasm", loading: "fetch" },
          release: false,
        },
      },
    });

    const transformed = server.transformModule(
      profile,
      entryId,
      typescript,
      runtimeId,
    );
    const diagnostics = [...transformed.diagnostics, ...server.check(profile)];
    if (diagnostics.length)
      throw new Error([...new Set(diagnostics)].join("\n"));

    const source = new RolldownMagicString(typescript);
    for (const edit of transformed.edits) {
      if (edit.start === edit.end) source.appendLeft(edit.start, edit.text);
      else source.overwrite(edit.start, edit.end, edit.text);
    }

    const runtime = `import __playgroundWasmUrl from ${JSON.stringify(wasmModuleId)};\n${server
      .renderJsFfi(profile, wasmModuleId)
      .replaceAll(JSON.stringify(wasmModuleId), "__playgroundWasmUrl")}`;
    const built = server.build(profile);
    if (!built.binFfiPath) {
      throw new Error(
        built.diagnostics.join("\n") ||
          "The Rust build produced no WebAssembly module",
      );
    }
    bundle = await rolldown({
      input: entryId,
      platform: "browser",
      transform: { define: { console: `globalThis.${EXECUTION_CONSOLE}` } },
      external: [wasmModuleId],
      plugins: [
        {
          name: "rustier-playground",
          resolveId(id) {
            if (id === entryId || id === runtimeId || id === "rustier")
              return id;
            throw new Error(`Unsupported playground import: ${id}`);
          },
          async load(id) {
            if (id === entryId)
              return { code: source.toString(), moduleType: "ts" };
            if (id === runtimeId) return runtime;
            if (id === "rustier") {
              return readFile(
                new URL(import.meta.resolve("rustier")),
                "utf8",
              );
            }
            throw new Error(`Unknown playground module: ${id}`);
          },
        },
      ],
    });

    const [wasm, { output }] = await Promise.all([
      readFile(built.binFfiPath),
      bundle.generate({ format: "es", codeSplitting: false }),
    ]);

    return {
      type: "ok",
      wasm: wasm.toString("base64"),
      bundle: output[0].code,
      wasmModuleId,
      duration: performance.now() - started,
    };
  } catch (cause) {
    const message = cause instanceof Error ? cause.message : String(cause);
    return {
      type: "err",
      message: message
        .replaceAll(`${root}${path.sep}`, "")
        .replaceAll(root, "."),
      duration: performance.now() - started,
    };
  } finally {
    await bundle?.close();
  }
}
