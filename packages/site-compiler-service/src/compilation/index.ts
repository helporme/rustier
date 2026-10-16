import {
  QUEUE_WAIT_MS,
  type CompilationRequest,
  type CompilerResult,
} from "../../../site/src/lib/playground/compilation-schema.ts";
import { ProcessWorker } from "./process-worker.ts";
import { probeSandbox } from "./sandbox.ts";

/** Admission failed because the session or bounded queue is already occupied. */
export class CompilationBusy extends Error {}

type Seat = {
  worker?: ProcessWorker;
  sessionId?: string;
  busy: boolean;
  idleSince: number;
  expiry?: NodeJS.Timeout;
};

type Job = {
  request: CompilationRequest;
  signal?: AbortSignal;
  resolve: (result: CompilerResult) => void;
  reject: (cause: Error) => void;
  timer?: NodeJS.Timeout;
  abort?: () => void;
};

/**
 * Owns warmed, isolated workers with best-effort session affinity.
 * Idle leases may be reclaimed; a sandbox is never reused across sessions.
 */
class CompilationPool {
  readonly #seats: Seat[];
  readonly #queue: Job[] = [];
  readonly #sessions = new Set<string>();
  readonly #running = new Set<Promise<void>>();
  readonly #lifetime = new AbortController();
  readonly #idleMs: number;
  #closed = false;

  /** Reads worker count and idle lease duration from the service environment. */
  constructor() {
    const size = Number(process.env.COMPILER_WORKERS ?? 2);
    this.#idleMs = Number(process.env.COMPILER_IDLE_MS ?? 120_000);

    if (!Number.isInteger(size) || size < 1 || size > 8)
      throw new Error("COMPILER_WORKERS must be between 1 and 8");
    if (!Number.isSafeInteger(this.#idleMs) || this.#idleMs < 1)
      throw new Error("COMPILER_IDLE_MS must be a positive integer");

    this.#seats = Array.from({ length: size }, () => ({
      busy: false,
      idleSince: 0,
    }));
  }

  /** Probes isolation and warms every seat, cleaning up all workers if startup fails. */
  async start(): Promise<void> {
    probeSandbox();

    const warming = this.#seats.map(async (seat) => {
      seat.worker = await ProcessWorker.start(this.#lifetime.signal);
    });

    try {
      await Promise.all(warming);
    } catch (cause) {
      this.#lifetime.abort();
      await Promise.allSettled(warming);
      await this.close();
      throw cause;
    }
  }

  /**
   * Queues one build per session, reusing its idle seat when available
   * Queue capacity and wait time are bounded; cancellation also stops active work.
   */
  schedule(
    request: CompilationRequest,
    signal?: AbortSignal,
  ): Promise<CompilerResult> {
    if (this.#closed)
      return Promise.reject(new Error("Compiler service is shutting down"));
    if (signal?.aborted)
      return Promise.reject(new Error("Compilation cancelled"));
    if (this.#sessions.has(request.sessionId))
      return Promise.reject(
        new CompilationBusy("This session already has a compilation pending"),
      );
    if (this.#queue.length >= 8)
      return Promise.reject(
        new CompilationBusy("The compilation queue is full"),
      );

    this.#sessions.add(request.sessionId);

    return new Promise<CompilerResult>((resolve, reject) => {
      const job: Job = { request, signal, resolve, reject };
      const remove = (cause: Error) => {
        const index = this.#queue.indexOf(job);

        if (index === -1) return;

        this.#queue.splice(index, 1);
        this.#detach(job);
        this.#sessions.delete(request.sessionId);
        reject(cause);
      };

      job.timer = setTimeout(
        () =>
          remove(
            new CompilationBusy(
              "The compilation queue wait exceeded 15 seconds",
            ),
          ),
        QUEUE_WAIT_MS,
      );
      job.abort = () => remove(new Error("Compilation cancelled"));
      signal?.addEventListener("abort", job.abort, { once: true });
      this.#queue.push(job);
      this.#drain();
    });
  }

  #detach(job: Job): void {
    clearTimeout(job.timer);

    if (job.abort) job.signal?.removeEventListener("abort", job.abort);
  }

  #drain(): void {
    while (!this.#closed && this.#queue.length) {
      const job = this.#queue[0];
      const available = this.#seats.filter((seat) => !seat.busy);
      const seat =
        available.find((seat) => seat.sessionId === job.request.sessionId) ??
        available.find((seat) => !seat.sessionId) ??
        available.sort((a, b) => a.idleSince - b.idleSince)[0];

      if (!seat) return;

      this.#queue.shift();
      this.#detach(job);
      clearTimeout(seat.expiry);
      seat.busy = true;

      const running = this.#execute(seat, job);
      this.#running.add(running);
      void running.finally(() => this.#running.delete(running));
    }
  }

  async #execute(seat: Seat, job: Job): Promise<void> {
    const signal = AbortSignal.any(
      job.signal
        ? [job.signal, this.#lifetime.signal]
        : [this.#lifetime.signal],
    );

    try {
      signal.throwIfAborted();

      if (
        (seat.sessionId && seat.sessionId !== job.request.sessionId) ||
        (seat.worker && !seat.worker.healthy)
      ) {
        await seat.worker?.dispose();
        seat.worker = undefined;
        seat.sessionId = undefined;
      }

      seat.worker ??= await ProcessWorker.start(signal);
      seat.sessionId = job.request.sessionId;
      job.resolve(await seat.worker.compile(job.request, signal));
    } catch (cause) {
      await seat.worker?.dispose();
      seat.worker = undefined;
      seat.sessionId = undefined;
      job.reject(
        cause instanceof Error ? cause : new Error("Compilation failed"),
      );
    } finally {
      this.#sessions.delete(job.request.sessionId);
      seat.busy = false;
      seat.idleSince = Date.now();

      if (!this.#closed && seat.sessionId) {
        seat.expiry = setTimeout(() => this.#expire(seat), this.#idleMs);
        seat.expiry.unref();
      } else if (!this.#closed && !seat.worker) {
        this.#expire(seat);
      }

      this.#drain();
    }
  }

  #expire(seat: Seat): void {
    seat.busy = true;
    seat.sessionId = undefined;

    const resetting = (async () => {
      try {
        await seat.worker?.dispose();
        seat.worker = undefined;
        seat.worker = await ProcessWorker.start(this.#lifetime.signal);
      } catch (cause) {
        console.error("Compiler seat warm-up failed", cause);
      } finally {
        seat.busy = false;
        this.#drain();
      }
    })();

    this.#running.add(resetting);
    void resetting.finally(() => this.#running.delete(resetting));
  }

  /** Rejects queued builds, cancels active work, and waits for every worker to exit. */
  async close(): Promise<void> {
    this.#closed = true;
    this.#lifetime.abort();

    for (const seat of this.#seats) clearTimeout(seat.expiry);
    for (const job of this.#queue.splice(0)) {
      this.#detach(job);
      this.#sessions.delete(job.request.sessionId);
      job.reject(new Error("Compiler service is shutting down"));
    }

    await Promise.allSettled(this.#running);
    await Promise.all(this.#seats.map((seat) => seat.worker?.dispose()));
  }
}

let pool: CompilationPool | undefined;

/** Creates and warms the service-wide pool before HTTP requests are accepted. */
export async function startCompilationPool(): Promise<void> {
  pool = new CompilationPool();
  await pool.start();
}

/**
 * Submits a build to the initialized pool; admission failures reject with CompilationBusy.
 * Compiler diagnostics are returned as results, while worker failures reject.
 */
export function scheduleCompilation(
  request: CompilationRequest,
  signal?: AbortSignal,
): Promise<CompilerResult> {
  if (!pool) return Promise.reject(new Error("Compiler pool has not started"));

  return pool.schedule(request, signal);
}

/** Shuts down the service-wide pool and its isolated processes. */
export async function stopCompilationPool(): Promise<void> {
  await pool?.close();
}
