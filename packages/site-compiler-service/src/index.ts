import { startServer } from "./server.ts";
import { runWorker } from "./compilation/worker.ts";
import { seed } from "./compilation/seed.ts";

switch (process.argv[2] ?? "server") {
  case "server":
    await startServer();
    break;
  case "worker":
    await runWorker();
    break;
  case "seed":
    await seed();
    break;
  default:
    throw new Error("Expected a service mode: server, worker or seed");
}
