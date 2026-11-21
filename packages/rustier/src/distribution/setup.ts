import type { DistributionPlan, PackageMetadata } from './plan'

export interface SetupMetadata extends PackageMetadata {
  private?: boolean
  dependencies?: Record<string, string>
  scripts?: Record<string, string>
}

/** Advisory checks only: custom release pipelines can satisfy these requirements separately. */
export function npmSetupWarnings(
  metadata: SetupMetadata,
  plan: DistributionPlan,
  packedFiles?: string[],
): string[] {
  if (plan.entry.type !== 'package' || !plan.packageDir) return []

  const warnings: string[] = []
  const name = plan.entry.specifier

  if (metadata.dependencies?.[name] !== metadata.version) {
    warnings.push(
      `The built library imports ${name}. In the root package.json, add or update this entry under "dependencies" (not "devDependencies"):\n`
      + `  ${JSON.stringify(name)}: ${JSON.stringify(metadata.version)}\n`
      + 'Use the exact version so consumers install the router built for this release.',
    )
  }

  const scriptPath = `${plan.packageDir}/publish.mjs`
  const escaped = scriptPath.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const invocation = new RegExp(`(?:^|&&|\\|\\||;)\\s*node(?:\\.exe)?(?:\\s+--[^\\s]+)*\\s+["']?(?:\\./)?${escaped}["']?(?=\\s|$|[;&|])`)

  const seen = new Set<string>()
  const invokesPublisher = (name: string): boolean => {
    if (seen.has(name)) return false
    seen.add(name)

    const script = metadata.scripts?.[name]
    if (!script) return false
    if (invocation.test(script.replaceAll('\\', '/'))) return true

    const calls = script.matchAll(/(?:^|&&|\|\||;)\s*(?:(?:npm|pnpm)\s+run(?:-script)?\s+|yarn\s+(?:run\s+)?)([\w:.-]+)/g)
    return [...calls].some(call => invokesPublisher(call[1]))
  }

  if (!invokesPublisher('prepublishOnly')) {
    const command = `node ./${scriptPath}`
    const existing = metadata.scripts?.prepublishOnly

    warnings.push(
      'No generated-package publish step was detected. For npm publish, set this entry under "scripts" in the root package.json:\n'
      + `  "prepublishOnly": ${JSON.stringify(existing ? `${existing} && ${command}` : command)}\n`
      + 'Skip this change if your release pipeline already publishes the target packages first, the router second, and the root package last.',
    )
  }

  if (packedFiles?.some(path => path === plan.packageDir || path.startsWith(`${plan.packageDir}/`))) {
    warnings.push(
      `The root tarball includes ${plan.packageDir}/, so consumers would download every platform binary. `
      + `Exclude that directory from the root package.json "files" list, or add /${plan.packageDir}/ to the root .npmignore if you don't use a "files" list. `
      + `Run npm pack --dry-run from the root and check that no ${plan.packageDir}/ files are listed.`,
    )
  }

  if (metadata.private) {
    warnings.push('The root package.json has "private": true, so npm will not publish it. '
      + 'If this package should stay private and you do not need separate npm packages, set distribution: "inline" in rustier(). '
      + 'Only remove "private": true if you intend to publish the root package.')
  }

  return warnings
}
