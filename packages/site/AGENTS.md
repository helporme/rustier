# SvelteKit site

- Keep pages focused on layout, view metadata, and user/URL interactions. Put application lifecycle and effects in playground classes; keep reusable UI components domain-independent and compose their content with snippets.
- Put static constants and exported types in `<script lang="ts" module>`. Keep controllers, IDs, and editable state instance-local.
- Prefer existing sources of truth and fewer concepts over additional flags, overrides, or wrappers. Use one consistent name per feature.
- Keep typography and colours in the global theme. Select palettes with `color-scheme`, not duplicated dark-only tokens.
- Reuse Monaco editors and cache models across tab/mode changes. Do not recreate editors or redefine themes merely because layout changes.
- Profile before adding performance workarounds. Avoid arbitrary delays, frame chains, and speculative warm-ups; remove ineffective workarounds rather than moving them elsewhere.
- Validate SSR, hydration, and interactive behaviour separately. Reproduce performance issues in the reported browser, and distinguish measured causes.
