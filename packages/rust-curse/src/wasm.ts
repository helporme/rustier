import { execSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { resolve, join } from 'node:path'

const WASM_BINDGEN_ROOT = '.rust-curse/wasm-bindgen-cli'

export interface BuildWasmOptions {
  /** Build in release mode. Defaults to false (debug). */
  release?: boolean
  /** Path to the Cargo.toml manifest. */
  manifestPath: string
  /** Output directory for the built wasm package. */
  outputDir: string
}

function ensureWasmTarget() {
  const installed = execSync('rustup target list --installed', { encoding: 'utf-8' })
  if (!installed.includes('wasm32-unknown-unknown')) {
    console.log('Installing wasm32-unknown-unknown target...')
    execSync('rustup target add wasm32-unknown-unknown', { stdio: 'inherit' })
  }
}

function ensureWasmBindgenCli(): string {
  const binPath = join(WASM_BINDGEN_ROOT, 'bin', 'wasm-bindgen')

  if (!existsSync(binPath)) {
    console.log('Installing wasm-bindgen-cli locally...')
    execSync(`cargo install wasm-bindgen-cli --root ${WASM_BINDGEN_ROOT}`, {
      stdio: 'inherit',
    })
  }

  return binPath
}

export function buildWasm(options: BuildWasmOptions) {
  ensureWasmTarget()
  const wasmBindgen = ensureWasmBindgenCli()

  const profile = options.release ? '--release' : ''
  const profileDir = options.release ? 'release' : 'debug'
  const manifestPath = resolve(options.manifestPath)
  const outputDir = resolve(options.outputDir)

  // Read crate name from manifest to find the output .wasm file
  const toml = readFileSync(manifestPath, 'utf-8')
  const nameMatch = toml.match(/^\s*name\s*=\s*"([^"]+)"/m)
  const crateName = (nameMatch?.[1] ?? 'rust_curse_wasm').replace(/-/g, '_')

  // Step 1: cargo build targeting wasm32-unknown-unknown
  execSync(
    `cargo build --target wasm32-unknown-unknown ${profile} --manifest-path ${manifestPath}`,
    { stdio: 'inherit' },
  )

  // Locate the compiled .wasm file
  const metadata = JSON.parse(
    execSync(`cargo metadata --format-version 1 --no-deps --manifest-path ${manifestPath}`, {
      encoding: 'utf-8',
    }),
  )
  const targetDir = metadata.target_directory
  const wasmFile = resolve(targetDir, `wasm32-unknown-unknown/${profileDir}/${crateName}.wasm`)

  // Step 2: wasm-bindgen to generate JS/TS bindings
  execSync(`${wasmBindgen} ${wasmFile} --out-dir ${outputDir}/pkg --target web`, {
    stdio: 'inherit',
  })
}
