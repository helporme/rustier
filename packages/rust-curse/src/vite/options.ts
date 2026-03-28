export type SourceOptions =
  | string
  | string[]
  | {
      include?: string | string[]
      exclude?: string | string[]
    }

export type RustCurseOptions = {
  source?: SourceOptions
}
