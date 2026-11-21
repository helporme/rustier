import process from 'node:process';
import { readFileSync } from 'node:fs';
function validate(runtime) {
  for (const name of ["__rustier_0702781657d797ac2692d1476652248b515287b53d301abbc86f78f593a16420","__rustier_272699e970adafcaa4fb5ec38d7dd462c1e119e7599d8ccb1beea84f72a95d34","__rustier_2c6b2fca332b927a03ebfff52e29ed7d775e0e1a7be47a9e5c97031a9af866b0","__rustier_7091187569c55db4cf8237718b2f05f9f7189551c86149d3c8cbad6f31edd743","__rustier_728f0c3a305e88e2aeb8cbdb59a33e930bce223e383895afff229191ce7ddc59","__rustier_ab85e2fa2ef1c04e81b07bffb31aad23bc53ddcec19d69e4b89dcfbae70f06ef","__rustier_b8d191727b94c0932759afd56243cb164076010f1224e6ef41c97c22f7e12342","__rustier_bb3f9615872f4a80a48b4d60c902483fac7b4bb6d79e3566d7f1eb6f231dfe93","__rustier_ccb66c37e8581afcbdbee8ba550b867999e8456e294a77b1d1ee792181541d9b","__rustier_e5fd93ef8335e08dcb72959b249adc12d18918757e7dff4703347cfeae18b86a","__rustier_efc3221a8e3681e5aae648616061ad3b51ba9c27be5f3939741856db7c5cdfcc","__rustier_fd9defce4c42fe9c8cafce27c536719a60c77420ca15556d59300da4d669d100","__rustier_fffe047fb26689589b00cc5dce55d16d3b73a904371e2c7ab06d391329f9684d"]) {
    if (!(name in runtime)) throw new Error("Rustier runtime is missing export " + name);
  }
  return runtime;
}
const routes = [{
  os: "darwin", cpu: "arm64", libc: null,
  load: () => import("rustier-bindings-darwin-arm64/node-js")
},
{
  os: "darwin", cpu: "x64", libc: null,
  load: () => import("rustier-bindings-darwin-x64/node-js")
},
{
  os: "linux", cpu: "x64", libc: "glibc",
  load: () => import("rustier-bindings-linux-x64-gnu/node-js")
},
{
  os: "linux", cpu: "arm64", libc: "glibc",
  load: () => import("rustier-bindings-linux-arm64-gnu/node-js")
},
{
  os: "linux", cpu: "x64", libc: "musl",
  load: () => import("rustier-bindings-linux-x64-musl/node-js")
},
{
  os: "linux", cpu: "arm64", libc: "musl",
  load: () => import("rustier-bindings-linux-arm64-musl/node-js")
},
{
  os: "win32", cpu: "x64", libc: null,
  load: () => import("rustier-bindings-win32-x64-msvc/node-js")
},
{
  os: "win32", cpu: "arm64", libc: null,
  load: () => import("rustier-bindings-win32-arm64-msvc/node-js")
}];
let libc;
if (process.platform === 'linux') {
  const report = process.report?.getReport();
  if (report?.header?.glibcVersionRuntime) libc = 'glibc';
  else if (report?.sharedObjects?.some(path => /(?:libc\.musl|ld-musl)/.test(path))) libc = 'musl';
  else {
    try { if (readFileSync('/usr/bin/ldd', 'utf8').includes('musl')) libc = 'musl'; } catch {}
  }
}
const route = routes.find(route => route.os === process.platform
  && route.cpu === process.arch && (!route.libc || route.libc === libc));
if (!route) throw new Error('No Rustier native runtime for ' + process.platform + '/' + process.arch + '/' + (libc ?? 'unknown'));
const runtime = validate(await route.load());
export const __rustier_0702781657d797ac2692d1476652248b515287b53d301abbc86f78f593a16420 = runtime["__rustier_0702781657d797ac2692d1476652248b515287b53d301abbc86f78f593a16420"];
export const __rustier_272699e970adafcaa4fb5ec38d7dd462c1e119e7599d8ccb1beea84f72a95d34 = runtime["__rustier_272699e970adafcaa4fb5ec38d7dd462c1e119e7599d8ccb1beea84f72a95d34"];
export const __rustier_2c6b2fca332b927a03ebfff52e29ed7d775e0e1a7be47a9e5c97031a9af866b0 = runtime["__rustier_2c6b2fca332b927a03ebfff52e29ed7d775e0e1a7be47a9e5c97031a9af866b0"];
export const __rustier_7091187569c55db4cf8237718b2f05f9f7189551c86149d3c8cbad6f31edd743 = runtime["__rustier_7091187569c55db4cf8237718b2f05f9f7189551c86149d3c8cbad6f31edd743"];
export const __rustier_728f0c3a305e88e2aeb8cbdb59a33e930bce223e383895afff229191ce7ddc59 = runtime["__rustier_728f0c3a305e88e2aeb8cbdb59a33e930bce223e383895afff229191ce7ddc59"];
export const __rustier_ab85e2fa2ef1c04e81b07bffb31aad23bc53ddcec19d69e4b89dcfbae70f06ef = runtime["__rustier_ab85e2fa2ef1c04e81b07bffb31aad23bc53ddcec19d69e4b89dcfbae70f06ef"];
export const __rustier_b8d191727b94c0932759afd56243cb164076010f1224e6ef41c97c22f7e12342 = runtime["__rustier_b8d191727b94c0932759afd56243cb164076010f1224e6ef41c97c22f7e12342"];
export const __rustier_bb3f9615872f4a80a48b4d60c902483fac7b4bb6d79e3566d7f1eb6f231dfe93 = runtime["__rustier_bb3f9615872f4a80a48b4d60c902483fac7b4bb6d79e3566d7f1eb6f231dfe93"];
export const __rustier_ccb66c37e8581afcbdbee8ba550b867999e8456e294a77b1d1ee792181541d9b = runtime["__rustier_ccb66c37e8581afcbdbee8ba550b867999e8456e294a77b1d1ee792181541d9b"];
export const __rustier_e5fd93ef8335e08dcb72959b249adc12d18918757e7dff4703347cfeae18b86a = runtime["__rustier_e5fd93ef8335e08dcb72959b249adc12d18918757e7dff4703347cfeae18b86a"];
export const __rustier_efc3221a8e3681e5aae648616061ad3b51ba9c27be5f3939741856db7c5cdfcc = runtime["__rustier_efc3221a8e3681e5aae648616061ad3b51ba9c27be5f3939741856db7c5cdfcc"];
export const __rustier_fd9defce4c42fe9c8cafce27c536719a60c77420ca15556d59300da4d669d100 = runtime["__rustier_fd9defce4c42fe9c8cafce27c536719a60c77420ca15556d59300da4d669d100"];
export const __rustier_fffe047fb26689589b00cc5dce55d16d3b73a904371e2c7ab06d391329f9684d = runtime["__rustier_fffe047fb26689589b00cc5dce55d16d3b73a904371e2c7ab06d391329f9684d"];
