import { readFileSync, readdirSync } from 'node:fs'
import { posix, resolve } from 'node:path'

import { RolldownMagicString, type Plugin } from 'rolldown'
import { parseSync } from 'rolldown/utils'
import type { TsdownPlugin } from 'tsdown'

import { Compilation, buildBatch, checkDrivers, createBatch, hostTarget, removeIteration } from './compiler'
import { normalizeOptions, type EffectiveOptions, type RustierOptions } from './options'
import { binaryNames, planDistribution, type DistributionPlan, type RuntimeArtifact } from './distribution/plan'
import { readPackageMetadata, writePackages } from './distribution/packages'

export type { RustierOptions, DevOptions, CrateOptions, Distribution, BuildDriver, Target, TargetName, NativeTargetName, WasmTargetName, TargetOptions, Loading } from './options'

const runtimeModule = 'rustier:runtime'

/**
 * Creates a tsdown/Rolldown plugin that compiles Rust snippets and emits their runtimes.
 * npm distribution generates packages and configures the root manifest without publishing.
 * @example rustier()
 * @example rustier({ targets: 'recommended', distribution: 'npm' })
 */
export default function rustier(options: RustierOptions = {}): Plugin {
  let effective: EffectiveOptions
  let compilations: Compilation[] | undefined
  let plan: DistributionPlan | undefined
  let iteration = 0
  let currentIteration: number | undefined
  const retired = new Set<number>()
  const tracked = new Set<string>()
  const reportedSetupWarnings = new Set<string>()
  const root = resolve(options.rootDir ?? process.cwd())
  const outDir = resolve(root, '.rustier/build')
  let finalizeWithTsdown = false

  function finalizePackages(warn: (message: string) => void) {
    if (plan?.entry.type !== 'package') return
    const patch = typeof effective.distribution !== 'object' || effective.distribution.patchPackageJson !== false
    for (const warning of writePackages(root, plan, patch)) {
      if (reportedSetupWarnings.has(warning)) continue
      reportedSetupWarnings.add(warning)
      warn(warning)
    }
  }

  const plugin: TsdownPlugin = {
    name: 'rustier',

    tsdownConfig(config) {
      finalizeWithTsdown = true
      const existing = config.hooks
      config.hooks = async hooks => {
        if (typeof existing === 'function') await existing(hooks)
        else if (existing) hooks.addHooks(existing)

        // tsdown may regenerate package.json after Rolldown's writeBundle.
        hooks.hook('build:done', ({ options }) => finalizePackages(message => options.logger.warn(message)))
      }
    },

    buildStart() {
      plan = undefined
      reportedSetupWarnings.clear()
      if (compilations) return

      effective = normalizeOptions(options, this.meta.watchMode, hostTarget())
      checkDrivers(effective.targets)
      try {
        for (const name of readdirSync(resolve(outDir, 'artifacts'))) {
          if (/^(0|[1-9]\d*)$/.test(name) && Number(name) <= 0xffffffff) {
            retired.add(Number(name))
            iteration = Math.max(iteration, Number(name) + 1)
          }
        }
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
      }
      const groups = new Map<string, EffectiveOptions['targets']>()
      for (const target of effective.targets) {
        const group = groups.get(target.kind) ?? []
        group.push(target)
        groups.set(target.kind, group)
      }
      compilations = [...groups.values()].map(targets => new Compilation(targets, effective))
    },

    resolveId(id) {
      if (id === runtimeModule) return { id, external: true }
    },

    transform(content, path) {
      if (path.includes('/node_modules/') || /\.d\.[cm]?ts(?:\?|$)/.test(path)) return
      if (!/\.[cm]?[jt]sx?(?:\?|$)/.test(path) && !content.includes('rustier') && !tracked.has(path)) return
      tracked.add(path)

      let edits: [number, number, string][] | undefined
      for (const compilation of compilations!) {
        const result = compilation.transform(path, content, runtimeModule)
        if ('diagnostics' in result) return this.error(result.diagnostics.join('\n'))
        if (edits && JSON.stringify(edits) !== JSON.stringify(result.edits)) {
          return this.error(`Targets produced incompatible JS transforms for ${path}`)
        }
        edits = result.edits
      }
      if (!edits?.length) return

      const result = new RolldownMagicString(content)
      for (const [start, end, text] of edits) {
        if (start === end) result.appendLeft(start, text)
        else result.overwrite(start, end, text)
      }
      return {
        code: result.toString(),
        map: result.generateMap({ source: path, includeContent: true, hires: true }).toString(),
      }
    },

    buildEnd(error) {
      if (error) return

      const modules = [...this.getModuleIds()]
      const included = new Set(modules)
      for (const id of tracked) if (!included.has(id)) tracked.delete(id)

      const exports = compilations!.map(compilation => compilation.collectProgram(modules))
      if (exports.some(names => JSON.stringify(names) !== JSON.stringify(exports[0]))) {
        return this.error('Target runtimes expose different binding exports')
      }
      if (!exports[0].length) {
        if (currentIteration !== undefined) retired.add(currentIteration)
        currentIteration = undefined
        return
      }
      if (iteration > 0xffffffff) return this.error('Rustier build iteration limit reached')

      const buildIteration = iteration++
      retired.add(buildIteration)
      const batch = createBatch()
      for (const compilation of compilations!) compilation.prepare(batch, outDir)
      const outcomes = buildBatch(batch, buildIteration, effective.concurrency)
      for (const outcome of outcomes) {
        for (const warning of outcome.warnings) this.warn(warning)
      }

      const failures = outcomes.filter(outcome => outcome.error)
      if (failures.length) {
        return this.error(failures.map(outcome => `${outcome.target}: ${outcome.error}`).join('\n'))
      }
      const artifacts: RuntimeArtifact[] = outcomes.map(outcome => {
        const target = effective.targets.find(target => target.target === outcome.target)!
        const groupIndex = compilations!.findIndex(compilation => compilation.targets.some(entry => entry.target === target.target))
        const compilation = compilations![groupIndex]
        const binary = `./${binaryNames[target.kind]}`
        const node = compilation.renderRuntime(binary, 'node-js')
        return {
          target,
          exports: exports[groupIndex],
          binary: readFileSync(outcome.binary_path!),
          runtimes: {
            'node-js': node,
            web: target.kind === 'node' ? node : compilation.renderRuntime(binary, 'web'),
            module: target.kind === 'node' ? node : compilation.renderRuntime(binary, 'module'),
          },
        }
      })
      plan = planDistribution(effective, artifacts,
        effective.distribution === 'inline' ? undefined : readPackageMetadata(root))
      for (const file of plan.files) this.emitFile({ type: 'asset', fileName: file.path, source: file.contents })
      if (currentIteration !== undefined) retired.add(currentIteration)
      currentIteration = buildIteration
      retired.delete(buildIteration)
    },

    renderStart(output) {
      if (plan && output.format !== 'es') this.error('Rustier runtime assets require ES module output')
    },

    renderChunk(code, chunk) {
      if (!plan || !chunk.imports.includes(runtimeModule)) return
      const result = new RolldownMagicString(code)
      const relative = plan.entry.type === 'file'
        ? posix.relative(posix.dirname(chunk.fileName), plan.entry.specifier) : plan.entry.specifier
      const specifier = plan.entry.type === 'file' && !relative.startsWith('.') ? `./${relative}` : relative
      for (const statement of parseSync(chunk.fileName, code).program.body) {
        if (statement.type === 'ImportDeclaration' && statement.source.value === runtimeModule) {
          result.overwrite(statement.source.start, statement.source.end, JSON.stringify(specifier))
        }
      }
      return { code: result.toString(), map: result.generateMap({ hires: true }).toString() }
    },

    writeBundle() {
      if (!finalizeWithTsdown) finalizePackages(message => this.warn(message))
      for (const previous of retired) {
        try {
          removeIteration(outDir, previous)
          retired.delete(previous)
        } catch (error) {
          this.warn(`Could not remove Rustier build iteration ${previous}: ${String(error)}`)
        }
      }
    },

    closeWatcher() {
      compilations = undefined
      tracked.clear()
    },
  }

  return plugin
}
