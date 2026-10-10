import {
  cp,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  rm,
  stat,
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, dirname, join } from 'node:path'
import { expect, inject, onTestFinished } from 'vitest'

declare module 'vitest' {
  interface ProvidedContext {
    regenerateCliSnapshots: boolean
  }
}

export interface SnapshotOptions {
  variants?: readonly string[]
}

async function readTree(directory: string) {
  const tree: Record<string, Buffer | null> = {}

  async function visit(path: string, prefix: string) {
    const entries = await readdir(path, { withFileTypes: true }).catch(
      (error: NodeJS.ErrnoException) => {
        if (path === directory && error.code === 'ENOENT') {
          return []
        }

        throw error
      },
    )

    for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
      const relative = `${prefix}${entry.name}`
      const absolute = join(path, entry.name)

      if (entry.isDirectory()) {
        tree[`${relative}/`] = null
        await visit(absolute, `${relative}/`)
      } else if (entry.isFile()) {
        tree[relative] = await readFile(absolute)
      } else {
        throw new Error(`Unsupported fixture entry: ${absolute}`)
      }
    }
  }

  await visit(directory, '')

  return tree
}

export async function snapshot(
  name: string,
  { variants = [] }: SnapshotOptions = {},
) {
  const testPath = expect.getState().testPath

  if (!testPath) {
    throw new Error('snapshot() must be called from a running test')
  }

  const directory = dirname(testPath)
  const temporary = await mkdtemp(join(tmpdir(), 'rustier-cli-'))
  const dir = join(temporary, 'project')
  const snapshotName = variants.length
    ? `${name}-with-${variants.join('-')}`
    : name
  const stored = join(directory, 'snapshots', snapshotName)

  onTestFinished(() => rm(temporary, { recursive: true, force: true }))

  await mkdir(dir)

  const project = join(directory, 'project')
  const base = await stat(project).catch((error: NodeJS.ErrnoException) => {
    if (error.code === 'ENOENT') {
      return undefined
    }

    throw error
  })

  const filter = (source: string) =>
    !['node_modules', 'target'].includes(basename(source))

  if (base) {
    await cp(project, dir, { recursive: true, filter })
  }

  for (const variant of variants) {
    await cp(join(directory, 'variants', variant), dir, {
      recursive: true,
      filter,
    })
  }

  return {
    dir,
    async assert() {
      if (inject('regenerateCliSnapshots')) {
        await rm(stored, { recursive: true, force: true })
        await cp(dir, stored, { recursive: true })
      }

      expect(await readTree(dir)).toEqual(await readTree(stored))
    },
  }
}
