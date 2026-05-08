// Types deliberately copied between vite and tsdown plugins, so developers can check all options in one place

export type SourcesOptions =
  | string
  | string[]
  | {
      include?: string | string[]
      exclude?: string | string[]
    }

export type RustCurseOptions = {
  /**
   * Project root directory
   * @default process.cwd()
   */
  rootDir?: string

  /**
   * Path to the temporary directory used for development.
   *
   * @default './.rust-curse'
   */
  outDir?: string

  /**
   * Js side configuration
   */
  js: {
    /**
     * Files to scan for ESM imports/exports.
     *
     * Based on the imports, rust code will be "treeshaked", so marked as dead.
     *
     * Defaults to all `.js` and `.ts` files under `src/`.
     */
    filter?: SourcesOptions

    /**
     * Generate ts types? True by default.
     */
    typescript?: boolean
  }

  /**
   * Rust side configuration
   */
  rust?: {
    /**
     * Root folder relative to which Rust modules will be registered.
     *
     * For example, for the following directory structure:
     * ```
     * src/
     *  utils/
     *    print.rs
     *  hello.rs
     * ```
     *
     * Setting this value to `./src` makes `print.rs` accessible via `use crate::utils::print` and `hello.rs` via just 'crate::hello'.
     *
     * @default './src'
     */
    moduleDir?: string
  }

  /**
   * Target platform.
   *
   * @default 'wasm'
   */
  platform?: 'wasm' | 'node'
}

export const defaultOptions: RustCurseOptions = {
  rootDir: process.cwd(),
  outDir: './.rust-curse',
  js: {
    filter: ['./src/**/*.js', './src/**/*.ts'],
  },
  rust: {
    moduleDir: './src',
  },
  platform: 'wasm',
}
