import type { NativePlatform } from '../targets'

export interface NativeRoute extends NativePlatform {
  module: string
}

export interface RouterPlan {
  exports: string[]
  native: NativeRoute[]
  wasm?: { module: string, web: string, node: string }
}

function forwarding(exports: string[]): string {
  for (const name of exports) {
    if (!/^[$A-Z_a-z][$\w]*$/.test(name)) throw new Error(`Invalid runtime export: ${name}`)
  }

  return exports.map(name => `export const ${name} = runtime[${JSON.stringify(name)}];`).join('\n')
}

export function renderRouter(plan: RouterPlan): Record<string, string> {
  const expected = JSON.stringify(plan.exports)
  const validate = `function validate(runtime) {
  for (const name of ${expected}) {
    if (!(name in runtime)) throw new Error("Rustier runtime is missing export " + name);
  }
  return runtime;
}`

  const files: Record<string, string> = {}

  if (plan.native.length) {
    const routes = plan.native.map(route => `{
  os: ${JSON.stringify(route.os)}, cpu: ${JSON.stringify(route.cpu)}, libc: ${JSON.stringify(route.libc ?? null)},
  load: () => import(${JSON.stringify(route.module)})
}`).join(',\n')

    files['node.mjs'] = `import process from 'node:process';
import { readFileSync } from 'node:fs';
${validate}
const routes = [${routes}];
let libc;
if (process.platform === 'linux') {
  const report = process.report?.getReport();
  if (report?.header?.glibcVersionRuntime) libc = 'glibc';
  else if (report?.sharedObjects?.some(path => /(?:libc\\.musl|ld-musl)/.test(path))) libc = 'musl';
  else {
    try { if (readFileSync('/usr/bin/ldd', 'utf8').includes('musl')) libc = 'musl'; } catch {}
  }
}
const route = routes.find(route => route.os === process.platform
  && route.cpu === process.arch && (!route.libc || route.libc === libc));
if (!route) throw new Error('No Rustier native runtime for ' + process.platform + '/' + process.arch + '/' + (libc ?? 'unknown'));
const runtime = validate(await route.load());
${forwarding(plan.exports)}
`
  }

  if (plan.wasm) {
    for (const [name, module] of Object.entries(plan.wasm)) {
      files[`wasm-${name}.mjs`] = `export { ${plan.exports.join(', ')} } from ${JSON.stringify(module)};\n`
    }
  }
  const candidates = [
    ...(plan.native.length ? ['./node.mjs'] : []),
    ...(plan.wasm ? ['./wasm-node.mjs', './wasm-web.mjs', './wasm-module.mjs'] : []),
  ]

  files['fallback.mjs'] = `${validate}
const candidates = [${candidates.map(module => `{ name: ${JSON.stringify(module)}, load: () => import(${JSON.stringify(module)}) }`).join(',\n')}];
const failures = [];
let runtime;
for (const candidate of candidates) {
  try {
    runtime = validate(await candidate.load());
    break;
  } catch (cause) {
    failures.push(new Error('Failed to load Rustier route ' + candidate.name, { cause }));
  }
}
if (!runtime) throw new AggregateError(failures, 'Unable to load a Rustier runtime in this environment');
${forwarding(plan.exports)}
`

  return files
}
