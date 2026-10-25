import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, existsSync, writeFileSync, rmSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import test from 'node:test'
import { rolldown, watch } from 'rolldown'
import rustier from '../dist/tsdown.mjs'

test('collects bundler-provided sources before compiling and shares one backend', async () => {
  const cwd = process.cwd()
  const scratch = resolve('.rustier')
  mkdirSync(scratch, { recursive: true })
  const root = mkdtempSync(join(scratch, 'plugin-test-'))
  process.chdir(root)
  try {
    const main = join(root, 'main.ts')
    const child = join(root, 'child.ts')
    const lazy = join(root, 'lazy.ts')
    const other = join(root, 'other.ts')
    const sources = new Map([
      [main, `
        import { rs } from 'rustier';
        import { identity } from './child.ts';
        export const echo = value => identity(rs\`\${value}\`);
        export const late = async () => (await import('./lazy.ts')).late();
      `],
      [child, `
        import { rs as rust } from 'rustier';
        export const identity = value => rust\`let value = \${value}; value\`;
      `],
      [lazy, `
        import { rs } from 'rustier';
        /* injected by another plugin */
      `],
      [other, `
        import { rs } from 'rustier';
        export const other = value => rs\`let other = \${value}; other\`;
      `],
    ])
    const plugin = rustier({ platform: { type: 'wasm', loading: 'node-js' } })
    let transformed = new Set()
    let compiling = false
    let builds = 0
    const buildEnd = plugin.buildEnd
    plugin.buildEnd = function (error) {
      assert.equal(compiling, false)
      for (const id of sources.keys()) assert.ok(transformed.has(id), id)
      compiling = true
      builds++
      return buildEnd.call(this, error)
    }
    const host = {
      name: 'test-source-provider',
      resolveId(id, importer) {
        if (id === 'rustier') return id
        const path = importer ? resolve(importer, '..', id) : id
        if (sources.has(path)) return path
      },
      load(id) {
        if (id === 'rustier') return 'export function rs() { throw new Error("uncompiled snippet"); }'
        return sources.get(id)
      },
      transform(code, id) {
        if (!sources.has(id)) return
        assert.equal(compiling, false, 'compilation must not happen during module loading')
        transformed.add(id)
        if (id === lazy) {
          return code.replace(
            '/* injected by another plugin */',
            'export const late = () => rs`__rustier_env.new_string("late module")`;',
          )
        }
      },
    }
    async function bundle(directory) {
      transformed = new Set()
      compiling = false
      const build = await rolldown({
        input: sources.has(other) ? { main, other } : main,
        platform: 'node',
        plugins: [host, plugin],
      })
      try {
        await build.write({
          dir: directory,
          format: 'esm',
          entryFileNames: 'entries/[name].mjs',
          chunkFileNames: 'chunks/[name]-[hash].mjs',
        })
      } finally {
        await build.close()
      }
      return import(pathToFileURL(join(directory, 'entries/main.mjs')).href)
    }

    const output = join(root, 'first')
    const first = await bundle(output)
    const value = { shared: true }
    assert.equal(first.echo(value), value)
    const otherEntry = await import(pathToFileURL(join(output, 'entries/other.mjs')).href)
    assert.equal(otherEntry.other(value), value)
    assert.equal(await first.late(), 'late module')
    assert.ok(WebAssembly.validate(readFileSync(join(output, 'bridge.wasm'))))
    assert.equal(builds, 1)

    // A fresh bundler build must not retain snippets from the previous graph.
    sources.delete(child)
    sources.delete(lazy)
    sources.delete(other)
    sources.set(main, `
      import { rs } from 'rustier';
      export const changed = () => rs\`__rustier_env.new_string("rebuilt")\`;
    `)
    const second = await bundle(join(root, 'second'))
    assert.equal(second.changed(), 'rebuilt')
    assert.equal(builds, 2)
    const artifacts = join(root, '.rustier/bridge/artifacts')
    assert.deepEqual(readdirSync(artifacts), ['1'])
    const crate = readFileSync(join(root, '.rustier/bridge/lib.rs'), 'utf8')
    assert.ok(!crate.includes('late module'))

    sources.set(main, "import { rs } from 'rustier'; export const bad = rs`missing_function()`;")
    await assert.rejects(bundle(join(root, 'failed')), /missing_function/)
    assert.equal(builds, 3)
    assert.deepEqual(readdirSync(artifacts), ['2'])
    assert.ok(!existsSync(join(root, 'failed/bridge.wasm')))
  } finally {
    process.chdir(cwd)
    rmSync(root, { recursive: true, force: true })
  }
})

test('watch rebuilds include unchanged modules', { timeout: 30_000 }, async () => {
  const cwd = process.cwd()
  const scratch = resolve('.rustier')
  mkdirSync(scratch, { recursive: true })
  const root = mkdtempSync(join(scratch, 'watch-test-'))
  process.chdir(root)
  let watcher
  try {
    const main = join(root, 'main.mjs')
    writeFileSync(join(root, 'child.mjs'), `
      import { rs } from 'rustier';
      export const child = () => rs\`__rustier_env.new_string("unchanged child")\`;
    `)
    const source = value => `
      import { rs } from 'rustier';
      export { child } from './child.mjs';
      export const main = () => rs\`__rustier_env.new_string("${value}")\`;
    `
    writeFileSync(main, source('first'))
    const output = join(root, 'dist')
    watcher = watch({
      input: main,
      platform: 'node',
      plugins: [{
        name: 'test-tag-import',
        resolveId(id) { if (id === 'rustier') return id },
        load(id) { if (id === 'rustier') return 'export const rs = () => {};' },
      }, rustier({ platform: { type: 'wasm', loading: 'node-js' } })],
      output: { dir: output, format: 'esm', entryFileNames: '[name].mjs' },
    })
    const events = []
    let notify
    watcher.on('event', event => {
      if (event.code === 'END' || event.code === 'ERROR') {
        events.push(event)
        notify?.()
      }
    })
    async function completed() {
      while (!events.length) await new Promise(resolve => { notify = resolve })
      const event = events.shift()
      if (event.code === 'ERROR') throw event.error
    }
    await completed()
    const first = await import(`${pathToFileURL(join(output, 'main.mjs')).href}?first`)
    assert.equal(first.main(), 'first')
    assert.equal(first.child(), 'unchanged child')
    writeFileSync(main, source('second'))
    await completed()
    const second = await import(`${pathToFileURL(join(output, 'main.mjs')).href}?second`)
    assert.equal(second.main(), 'second')
    assert.equal(second.child(), 'unchanged child')
    assert.deepEqual(readdirSync(join(root, '.rustier/bridge/artifacts')), ['1'])
  } finally {
    await watcher?.close()
    process.chdir(cwd)
    rmSync(root, { recursive: true, force: true })
  }
})

test('skips plain JS builds and supports the fetch loader', async () => {
  const cwd = process.cwd()
  const scratch = resolve('.rustier')
  mkdirSync(scratch, { recursive: true })
  const root = mkdtempSync(join(scratch, 'fetch-test-'))
  process.chdir(root)
  const originalFetch = globalThis.fetch
  try {
    const main = join(root, 'main.ts')
    let source = 'export const plain = 42;'
    const plugin = rustier({ platform: { type: 'wasm', loading: 'web' } })
    const host = {
      name: 'test-source-provider',
      resolveId(id) { if (id === main || id === 'rustier') return id },
      load(id) {
        if (id === main) return source
        if (id === 'rustier') return 'export const rs = () => {};'
      },
    }
    async function bundle(directory) {
      const build = await rolldown({ input: main, plugins: [host, plugin] })
      try {
        await build.write({ dir: directory, format: 'esm', entryFileNames: '[name].mjs' })
      } finally {
        await build.close()
      }
      return import(pathToFileURL(join(directory, 'main.mjs')).href)
    }
    assert.equal((await bundle(join(root, 'plain'))).plain, 42)
    assert.ok(!existsSync(join(root, '.rustier/bridge')))
    assert.ok(!existsSync(join(root, 'plain/bridge.wasm')))
    source = "import { rs } from 'rustier'; export const echo = value => rs`${value}`;"
    let requests = 0
    globalThis.fetch = async url => {
      assert.equal(url.protocol, 'file:')
      requests++
      return new Response(readFileSync(url))
    }
    const module = await bundle(join(root, 'fetch'))
    const value = { same: true }
    assert.equal(module.echo(value), value)
    assert.equal(requests, 1)
  } finally {
    globalThis.fetch = originalFetch
    process.chdir(cwd)
    rmSync(root, { recursive: true, force: true })
  }
})
