import * as v from "valibot";
import {
  BUILD_TIMEOUT_MS,
  MAX_OUTPUT_BYTES,
  WORKER_START_TIMEOUT_MS,
} from "../../../site/src/lib/playground/compilation/limits.ts";
import {
  compilerResultSchema,
  type BuildInput,
  type CompilerResult,
} from "../../../site/src/lib/playground/compilation/schema.ts";
import { spawnWorker } from "./sandbox.ts";

/** Supervises one persistent sandboxed compiler over bounded JSON-line messages. */
export class ProcessWorker {
  readonly #child = spawnWorker();
  readonly #closed: Promise<void>;
  #exited = false;
  #stopped = false;
  #buffer = "";
  #stderrBytes = 0;
  #pending?: {
    kind: "ready" | "result";
    resolve: (result?: CompilerResult) => void;
    reject: (cause: Error) => void;
  };

  get healthy(): boolean {
    return !this.#exited && !this.#stopped;
  }

  /** Spawns the process and attaches supervision; use start() to await readiness. */
  constructor() {
    this.#closed = new Promise((resolve) =>
      this.#child.once("close", () => {
        this.#exited = true;
        this.#pending?.reject(new Error("The compiler worker exited"));
        resolve();
      }),
    );

    this.#child.once("error", (cause) => this.#pending?.reject(cause));
    this.#child.stdin.on("error", (cause) => this.#pending?.reject(cause));
    this.#child.stdout.setEncoding("utf8");
    this.#child.stdout.on("data", (chunk: string) => {
      this.#buffer += chunk;

      if (Buffer.byteLength(this.#buffer) > MAX_OUTPUT_BYTES) {
        this.#pending?.reject(new Error("Compilation output exceeded 16 MiB"));
        void this.dispose();
        return;
      }

      let newline: number;

      while ((newline = this.#buffer.indexOf("\n")) !== -1) {
        const line = this.#buffer.slice(0, newline);
        this.#buffer = this.#buffer.slice(newline + 1);

        try {
          const message: unknown = JSON.parse(line);

          if (!this.#pending) throw new Error("Unexpected compiler output");

          if (this.#pending.kind === "ready") {
            if (!v.is(v.object({ type: v.literal("ready") }), message))
              throw new Error("Invalid compiler readiness message");

            this.#pending.resolve();
          } else {
            this.#pending.resolve(v.parse(compilerResultSchema, message));
          }
        } catch {
          this.#pending?.reject(new Error("Invalid compiler output"));
          void this.dispose();
        }
      }
    });
    this.#child.stderr.on("data", (chunk: Buffer) => {
      this.#stderrBytes += chunk.length;

      if (this.#stderrBytes > 64 * 1024) {
        this.#pending?.reject(
          new Error("Compiler diagnostics exceeded 64 KiB"),
        );
        void this.dispose();
      }
    });
  }

  /** Waits for compiler warm-up and destroys the sandbox if startup fails or is cancelled. */
  static async start(signal?: AbortSignal): Promise<ProcessWorker> {
    const worker = new ProcessWorker();

    try {
      await worker.#wait("ready", WORKER_START_TIMEOUT_MS, signal);
      return worker;
    } catch (cause) {
      await worker.dispose();
      throw cause;
    }
  }

  /**
   * Runs one build against the retained compiler state, enforcing its deadline.
   * The pool serializes calls and disposes the worker after rejection.
   */
  async compile(
    input: BuildInput,
    signal?: AbortSignal,
  ): Promise<CompilerResult> {
    const result = this.#wait("result", BUILD_TIMEOUT_MS, signal);

    this.#child.stdin.write(`${JSON.stringify(input)}\n`);

    return (await result)!;
  }

  #wait(kind: "ready" | "result", timeout: number, signal?: AbortSignal) {
    let timer: NodeJS.Timeout | undefined;
    let abort: (() => void) | undefined;

    const result = new Promise<CompilerResult | undefined>(
      (resolve, reject) => {
        if (!this.healthy || this.#pending) {
          reject(new Error("Compiler worker is unavailable"));
          return;
        }

        this.#stderrBytes = 0;
        this.#pending = { kind, resolve, reject };
        timer = setTimeout(
          () => reject(new Error(`Compiler ${kind} exceeded its deadline`)),
          timeout,
        );
        abort = () => reject(new Error("Compilation cancelled"));
        signal?.addEventListener("abort", abort, { once: true });

        if (signal?.aborted) abort();
      },
    );

    return result.finally(() => {
      clearTimeout(timer);

      if (abort) signal?.removeEventListener("abort", abort);

      this.#pending = undefined;
    });
  }

  /** Kills the process group and waits for exit; repeated calls are safe. */
  async dispose(): Promise<void> {
    if (!this.#exited && !this.#stopped && this.#child.pid) {
      this.#stopped = true;

      try {
        process.kill(-this.#child.pid, "SIGKILL");
      } catch (cause) {
        if ((cause as NodeJS.ErrnoException).code !== "ESRCH") throw cause;
      }
    }

    await this.#closed;
  }
}
