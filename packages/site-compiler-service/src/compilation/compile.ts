import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import { rolldown } from "rolldown";
import {
  Server,
  type ServerDiagnostic,
  type ServerDiagnosticSeverity,
  type ServerPlatform,
  type ServerWasmLoading,
} from "rustier/unstable";
import { EXECUTION_CONSOLE } from "../../../site/src/lib/playground/compilation/execution-protocol.ts";
import type {
  BuildInput,
  CompilerResult,
} from "../../../site/src/lib/playground/compilation/schema.ts";

const root = path.resolve(process.env.PLAYGROUND_ROOT ?? "/work/project");
const source = path.join(root, "src", "lib.rs");
const manifest = `[package]
name = "playground"
version = "0.0.0"
edition = "2024"
publish = false
build = false

[workspace]
`;
const platform: ServerPlatform = {
  type: "Wasm",
  target: 0 as ServerWasmLoading.Fetch,
};
const entryId = "\0playground.ts";
const bindingsLoaderId = "\0playground-bindings";
const wasmModuleId = "playground:wasm";
let server: Server | undefined;

export async function compile({ code, typescript }: BuildInput): Promise<CompilerResult> {
  const started = performance.now();
  const failure = (diagnostics: ServerDiagnostic[]): CompilerResult => ({
    type: "err",
    message: diagnostics
      .map((diagnostic) =>
        diagnostic.rendered
          .replaceAll(`${root}${path.sep}`, "")
          .replaceAll(root, "."),
      )
      .join("\n"),
    duration: performance.now() - started,
  });

  await mkdir(path.dirname(source), { recursive: true });
  await writeFile(source, code);

  await writeFile(path.join(root, "Cargo.toml"), manifest);

  if (server) {
    const changed = server.change({ filePath: source, contents: code });

    if (changed.type === "Err") return failure([changed.error]);
  } else {
    const created = Server.create({
      rootDir: root,
      manifestPath: "Cargo.toml",
      ts: false,
    });

    if (created.type === "Err") return failure([created.error]);

    server = created.value;
  }

  const errors = server
    .check()
    .filter(
      (diagnostic) =>
        diagnostic.severity === (0 as ServerDiagnosticSeverity.Error),
    );

  if (errors.length > 0) return failure(errors);

  const built = server.build({ platform, release: false });

  if (built.type === "Err") return failure([built.error]);

  const [binary] = built.value.binPaths;

  if (!binary) throw new Error("The Rust build produced no WebAssembly module");

  const bindingsPath = binary.replace(/_bg\.wasm$/, ".js");
  const artifactDir = await realpath(path.dirname(binary));
  const isGeneratedJavaScript = (file: string) => {
    const relative = path.relative(artifactDir, file);
    return (
      relative !== "" &&
      relative !== ".." &&
      !relative.startsWith(`..${path.sep}`) &&
      !path.isAbsolute(relative) &&
      /\.(?:js|mjs)$/.test(file)
    );
  };
  const loaded = server;
  const modules = loaded.modules();
  const rootModule = modules.find((module) => module.isRoot)?.path;
  let bundle: Awaited<ReturnType<typeof rolldown>> | undefined;

  try {
    bundle = await rolldown({
      input: entryId,
      platform: "browser",
      transform: { define: { console: `globalThis.${EXECUTION_CONSOLE}` } },
      external: [wasmModuleId],
      plugins: [
        {
          name: "rustier-playground",
          async resolveId(id, importer) {
            if (id === rootModule || id === "./lib.rs") return source;
            if (id === bindingsPath && importer !== bindingsLoaderId)
              return bindingsLoaderId;

            if (
              id === entryId ||
              id === source ||
              id === bindingsLoaderId ||
              id === "$rust" ||
              id.startsWith("$rust/")
            )
              return id;

            const file = path.isAbsolute(id)
              ? id
              : importer && isGeneratedJavaScript(importer) && id.startsWith(".")
                ? path.resolve(path.dirname(importer), id)
                : undefined;

            if (file && isGeneratedJavaScript(file)) {
              const canonical = await realpath(file);
              if (isGeneratedJavaScript(canonical)) return canonical;
            }

            throw new Error(`Unsupported playground import: ${id}`);
          },
          async load(id) {
            if (id === entryId) {
              return { code: typescript, moduleType: "ts" };
            }
            if (id === bindingsLoaderId) {
              return `import wasmUrl from ${JSON.stringify(wasmModuleId)};
import init from ${JSON.stringify(bindingsPath)};
export * from ${JSON.stringify(bindingsPath)};
export default () => init({ module_or_path: wasmUrl });`;
            }
            if (isGeneratedJavaScript(id)) return readFile(id, "utf8");

            const module = loaded.load(platform, id);

            if (module == null) throw new Error(`Unknown Rust module ${id}`);

            return module;
          },
        },
      ],
    });

    const [wasm, { output }] = await Promise.all([
      readFile(binary),
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
    return {
      type: "err",
      message: cause instanceof Error ? cause.message : String(cause),
      duration: performance.now() - started,
    };
  } finally {
    await bundle?.close();
  }
}
