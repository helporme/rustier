import rolldown, { type RustierProfile } from './rolldown'

export type { Platform } from './rolldown'

export interface RustierOptions extends RustierProfile {}

export default function rustier(options: RustierOptions) {
  return rolldown({
    profiles: { default: options },
    activeProfile: () => 'default',
  })
}
