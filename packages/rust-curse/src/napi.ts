import { NapiCli } from '@napi-rs/cli'

export interface BuildNapiOptions {
  /** Build in release mode. Defaults to false (debug). */
  release?: boolean
  /** Path to the Cargo.toml manifest. */
  manifestPath: string
  /** Output directory for the built .node file and JS binding. */
  outputDir: string
}

export async function buildNapi(options: BuildNapiOptions) {
  const cli = new NapiCli()
  const { task } = await cli.build({
    manifestPath: options.manifestPath,
    outputDir: options.outputDir,
    release: options.release ?? false,
    platform: true,
    esm: true,
    noDtsHeader: true,
    dtsCache: false,
  })
  return task
}
