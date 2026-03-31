// Types deliberately copied between vite and tsdown plugins, so developers can check all options in one place
//
export type SourcesOptions =
  | string
  | string[]
  | {
      include?: string | string[]
      exclude?: string | string[]
    }

export type RustCurseOptions = {
  /**
   * Path to the temporary directory used for development.
   *
   * @default './.rust-curse'
   */
  tempDir?: string

  /**
   * Files to scan for ESM imports/exports.
   *
   * Based on the imports, rust code will be "treeshaked", so marked as dead.
   *
   * Defaults to all `.js` and `.ts` files under `src/`.
   */
  sources?: SourcesOptions

  /**
   * Rust side configuration
   */
  rust?: {
    /**
     * Where `Cargo.toml` is located.
     * @default './Cargo.toml'
     */
    manifestPath?: string

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
   * SSR build configuration overrides
   */
  ssr?: {
    /**
     * Files to scan for ESM imports/exports in SSR.
     *
     * Based on the imports, rust code will be "treeshaked", so marked as dead.
     *
     * Includes to all `.js`, `.ts` files under `src/`.
     */
    sources?: SourcesOptions
  }
}

export const defaultOptions: RustCurseOptions = {
  tempDir: './.rust-curse',
  sources: ['./src/**/*.js', './src/**/*.ts'],
  rust: {
    manifestPath: './Cargo.toml',
    moduleDir: './src',
  },
  ssr: {
    sources: ['./src/**/*.js', './src/**/*.ts'],
  },
}
