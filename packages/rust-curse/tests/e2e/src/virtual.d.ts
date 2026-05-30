/**
 * Module generated at build time by vitest plugin containing all build warnings
 */
declare module '$build/warns' {
  export interface RustCurseWarnLabel {
    message?: string
    beginLine: number
    beginCol: number
    endLine: number
    endCol: number
  }

  export interface RustCurseWarn {
    code: string
    message: string
    sourceCodePath?: string
    label?: RustCurseWarnLabel
    help?: string
    rendered?: string
  }

  export const rustCurseWarns: RustCurseWarn[]
}

declare module '*?raw' {
  const text: string
  export default text
}
