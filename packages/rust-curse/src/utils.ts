import { SourceOptions } from 'rust-curse-native'

/**
 * `Required` type but also applied on properties.
 */
export type DeepRequired<T> = Required<{
  [K in keyof T]: DeepRequired<T[K]>
}>

type DeepMergeable = Record<string, unknown>

/**
 * Merges two or more objects into one.
 */
export function deepMerge<T extends DeepMergeable>(
  target: T,
  ...sources: DeepMergeable[]
): T {
  const result = { ...target }

  for (const source of sources) {
    for (const key of Object.keys(source)) {
      if (isObject(source[key]) && isObject(result[key])) {
        result[key as keyof T] = deepMerge(
          result[key] as DeepMergeable,
          source[key] as DeepMergeable,
        ) as T[keyof T]
      } else {
        result[key as keyof T] = source[key] as T[keyof T]
      }
    }
  }

  return result
}

function isObject(value: unknown): value is DeepMergeable {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

/**
 * Normalizes source options
 */
export function normalizeSources(
  options:
    | {
        include?: string | string[]
        exclude?: string | string[]
      }
    | string
    | string[],
): SourceOptions {
  if (typeof options === 'string') return { include: [options], exclude: [] }
  if (Array.isArray(options)) return { include: options, exclude: [] }

  const include =
    typeof options.include === 'string'
      ? [options.include]
      : (options.include ?? [])

  const exclude =
    typeof options.exclude === 'string'
      ? [options.exclude]
      : (options.exclude ?? [])

  return { include, exclude }
}
