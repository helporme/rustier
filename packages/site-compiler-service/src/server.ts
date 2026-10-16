import { timingSafeEqual } from "node:crypto";
import Fastify, { LogController } from "fastify";
import * as v from "valibot";
import {
  MAX_INPUT_BYTES,
  compilationRequestSchema,
} from "../../site/src/lib/playground/compilation-schema.ts";
import {
  CompilationBusy,
  scheduleCompilation,
  startCompilationPool,
  stopCompilationPool,
} from "./compilation/index.ts";

const token = process.env.COMPILER_TOKEN;

if (!token || Buffer.byteLength(token) < 32)
  throw new Error("COMPILER_TOKEN must contain at least 32 bytes");

const authorization = Buffer.from(`Bearer ${token}`);
const app = Fastify({
  bodyLimit: MAX_INPUT_BYTES,
  requestTimeout: 15_000,
  logger: true,
  logController: new LogController({ disableRequestLogging: true }),
});

app.server.headersTimeout = 10_000;

app.get("/health", () => ({ status: "ok" }));
app.post("/build", {
  onRequest: async (request, reply) => {
    const supplied = Buffer.from(request.headers.authorization ?? "");

    if (
      supplied.length !== authorization.length ||
      !timingSafeEqual(supplied, authorization)
    )
      return reply.code(401).send({ error: "Unauthorized" });
  },
  handler: async (request, reply) => {
    const parsed = v.safeParse(compilationRequestSchema, request.body);

    if (!parsed.success)
      return reply.code(400).send({ error: "Invalid compilation request" });

    const controller = new AbortController();
    const disconnect = () => {
      if (!reply.raw.writableFinished) controller.abort();
    };
    reply.raw.once("close", disconnect);

    try {
      return await scheduleCompilation(parsed.output, controller.signal);
    } catch (cause) {
      if (cause instanceof CompilationBusy)
        return reply
          .header("retry-after", "1")
          .code(429)
          .send({ error: cause.message });

      return reply.code(503).send({
        error:
          cause instanceof Error ? cause.message : "Compiler worker failed",
      });
    } finally {
      reply.raw.off("close", disconnect);
    }
  },
});

app.addHook("onClose", stopCompilationPool);

for (const signal of ["SIGTERM", "SIGINT"] as const) {
  process.once(signal, async () => {
    await stopCompilationPool();
    await app.close();
  });
}

await startCompilationPool();
await app.listen({ port: 3001, host: "0.0.0.0" });
