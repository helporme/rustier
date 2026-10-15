import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { rolldown } from "rolldown";
import {
  Server,
  type ServerDiagnostic,
  type ServerDiagnosticSeverity,
  type ServerPlatform,
  type ServerWasmLoading,
} from "rustier/unstable";
import type {
  BuildInput,
  CompilerResult,
} from "../../../site/src/lib/playground/compilation-schema.ts";

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
const ffiModuleId = "$rust/unstable!/wasm";
const entryId = "\0playground-entry";
let server: Server | undefined;

export async function compile({
  code,
  modules,
}: BuildInput): Promise<CompilerResult> {
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

  const loaded = server;
  const specifiers = ["./lib.rs", ...new Set(modules)];
  const exports = Object.fromEntries(
    specifiers.map((specifier, index) => [specifier, `m${index}`]),
  );
  const bundle = await rolldown({
    input: entryId,
    external: [ffiModuleId],
    plugins: [
      {
        name: "rustier-playground",
        resolveId(id) {
          if (
            id === entryId ||
            id === source ||
            (id.startsWith("$rust/") && id !== ffiModuleId)
          )
            return id;

          return null;
        },
        load(id) {
          if (id === entryId) {
            return specifiers
              .map(
                (specifier) =>
                  `export * as ${exports[specifier]} from ${JSON.stringify(specifier === "./lib.rs" ? source : specifier)};`,
              )
              .join("\n");
          }

          const module = loaded.load(platform, id);

          if (module == null) throw new Error(`Unknown Rust module ${id}`);

          return module;
        },
      },
    ],
  });

  try {
    const [[wasm, glue], { output }] = await Promise.all([
      Promise.all([
        readFile(binary),
        readFile(binary.replace(/_bg\.wasm$/, ".js"), "utf8"),
      ]),
      bundle.generate({ format: "es" }),
    ]);

    return {
      type: "ok",
      wasm: wasm.toString("base64"),
      glue,
      bundle: output[0].code,
      ffiModuleId,
      exports,
      duration: performance.now() - started,
    };
  } catch (cause) {
    return {
      type: "err",
      message: cause instanceof Error ? cause.message : String(cause),
      duration: performance.now() - started,
    };
  } finally {
    await bundle.close();
  }
}
