import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

function linuxLibc() {
  try {
    const report = process.report?.getReport();
    if (report?.header?.glibcVersionRuntime) return "gnu";
    if (report?.sharedObjects?.some((file) => /(?:libc\.musl-|ld-musl-)/.test(file))) {
      return "musl";
    }
  } catch {
    // Reports may be disabled; inspect ldd without spawning a subprocess.
  }
  try {
    const ldd = require("node:fs").readFileSync("/usr/bin/ldd", "utf8");
    if (ldd.includes("musl")) return "musl";
    if (/GNU C Library|GLIBC|GNU libc/.test(ldd)) return "gnu";
  } catch {
    // Missing ldd is not evidence of either libc.
  }
  throw new Error(
    "Cannot determine Linux libc (GNU or musl) from process.report or /usr/bin/ldd; refusing to guess the native addon ABI.",
  );
}

function nativeTarget() {
  const { platform, arch } = process;
  switch (platform) {
    case "darwin":
    case "freebsd":
      if (["x64", "arm64"].includes(arch)) return `${platform}-${arch}`;
      break;
    case "android":
      if (arch === "arm64") return "android-arm64";
      if (arch === "arm") return "android-arm-eabi";
      break;
    case "openharmony":
    case "ohos":
      if (["x64", "arm64", "arm"].includes(arch)) return `openharmony-${arch}`;
      break;
    case "win32": {
      // Match napi-rs's Node-build heuristic; Node has no definitive Windows ABI API.
      const config = process.config?.variables;
      const gnu = config?.shlib_suffix === "dll.a" || config?.node_target_type === "shared_library";
      if (arch === "x64") return `win32-x64-${gnu ? "gnu" : "msvc"}`;
      if (["ia32", "arm64"].includes(arch)) return `win32-${arch}-msvc`;
      break;
    }
    case "linux":
      if (["x64", "arm64", "arm", "riscv64", "loong64", "ppc64", "s390x"].includes(arch)) {
        const libc = linuxLibc();
        if (libc === "musl" && ["ppc64", "s390x"].includes(arch)) {
          throw new Error(`Unsupported native addon ABI: linux/${arch}/${libc}`);
        }
        // Node's "arm" does not identify ARM ISA/float ABI: ship ARMv7 hard-float.
        return `linux-${arch}-${libc}${arch === "arm" ? "eabihf" : ""}`;
      }
      break;
  }
  throw new Error(`Unsupported native addon platform/architecture: ${platform}/${arch}`);
}

// Load exactly one sibling. Preserve Node's original missing-file/dlopen error.
const native = require(`./index.${nativeTarget()}.node`);
