import { rustCurseDiags } from '$build/diags'
import { inject } from 'vitest'

export const hasWarn = (code: string, includes: string) =>
  rustCurseDiags.some((w) => w.code === code && w.message.includes(includes))

export const makeBigInt = (value: string | number) =>
  (
    globalThis as unknown as { BigInt: (value: string | number) => bigint }
  ).BigInt(value)

export const target = inject('target' as never) as 'node' | 'wasm'
