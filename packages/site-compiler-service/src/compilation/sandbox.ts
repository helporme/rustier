import { spawn, spawnSync } from "node:child_process";

const args = [
  "--unshare-all",
  "--uid",
  "0",
  "--gid",
  "0",
  "--die-with-parent",
  "--clearenv",
  "--ro-bind",
  "/sandbox",
  "/",
  "--proc",
  "/proc",
  "--ro-bind",
  "/dev/null",
  "/proc/kcore",
  "--ro-bind",
  "/dev/null",
  "/proc/keys",
  "--remount-ro",
  "/proc",
  "--dev",
  "/dev",
  "--size",
  "1073741824",
  "--tmpfs",
  "/work",
  "--size",
  "67108864",
  "--tmpfs",
  "/tmp",
  "--chdir",
  "/app/packages/site-compiler-service",
  "--setenv",
  "HOME",
  "/work",
  "--setenv",
  "PATH",
  "/opt/cargo/bin:/usr/local/bin:/usr/bin:/bin",
  "--setenv",
  "RUSTUP_HOME",
  "/opt/rustup",
  "--setenv",
  "CARGO_HOME",
  "/work/cargo",
  "--setenv",
  "CARGO_NET_OFFLINE",
  "true",
  "--setenv",
  "CARGO_BUILD_JOBS",
  "1",
  "/usr/local/bin/node",
];

export function probeSandbox(): void {
  const probe = spawnSync("bwrap", [...args, "--version"], {
    encoding: "utf8",
    timeout: 10_000,
    env: { PATH: process.env.PATH },
  });

  if (probe.error || probe.status !== 0)
    throw new Error(
      `Compiler sandbox is unavailable: ${probe.error?.message ?? probe.stderr}`,
    );
}

export function spawnWorker() {
  return spawn("bwrap", [...args, "dist/index.mjs", "worker"], {
    detached: true,
    stdio: ["pipe", "pipe", "pipe"],
    env: { PATH: process.env.PATH },
  });
}
