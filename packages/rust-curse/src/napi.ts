import { NapiCli } from '@napi-rs/cli'
import { NapiBuildArgs, NapiBuildAritfact } from 'rust-curse-native'

export interface BuildNapiOptions {
  /** Build in release mode. Defaults to false (debug). */
  release?: boolean
  /** Path to the Cargo.toml manifest. */
  manifestPath: string
  /** Output directory for the built .node file and JS binding. */
  outputDir: string
}

const cli = new NapiCli()

export async function buildNapi(
  err: unknown,
  options: NapiBuildArgs,
): Promise<NapiBuildAritfact[]> {
  if (err) throw err

  const { task } = await cli.build({
    manifestPath: options.manifestAbsPath,
    outputDir: options.outputAbsDir,
    targetDir: options.targetAbsDir,
    release: options.release,
    // seems to be a little typo in napi, it expects a space separated list of features
    // instead of an array
    features: [options.features.join(' ')],
    platform: true,
    esm: true,
    noDtsHeader: true,
    dtsCache: false,
  })
  return task as Promise<NapiBuildAritfact[]>
}
