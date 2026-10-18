# Rustier agent guide

## General rules
- Do not add tests unless requested. You may add temporary tests during implementation, but remove them before finishing and notify the developer in a test coverage gap.

## Validation

- After making changes in this package, run `pnpm check` from `packages/site`. 

## Page rules

- Break the big `+page.svelte` on subcomponents. In case you need a subcomponent used specifically for this page, put it under `(components)/` folder near the `+page.svelte`.
- Keep pages focused on layout, view metadata, and user/URL interactions. Put application lifecycle and effects in separate classes with stateful fields or function with states inside under the `(components)` folder as well.

## Svelte rules

- Put static constants and exported types in `<script lang="ts" module>`. Keep controllers, IDs, and editable state instance-local.
- Profile before adding performance workarounds. Avoid arbitrary delays, frame chains, and speculative warm-ups; remove ineffective workarounds rather than moving them elsewhere.
- Validate SSR, hydration, and interactive behaviour separately using playwright or other tooling.

## Compoent rules

- Keep typography and colours in the global theme, do not add new ad-hoc anywhere without explicit developer consent.
- Keep reusable UI components inside `lib/ui/components` domain-independent and compose their content with snippets.
- Always use a `shadcn-svelte` for component basis, do not came up with your custom component first if there is some basic variant of it in `shadcn-svelte`. Use the cli to install the component and restyle it.
- Maintain a "UI Playground" page at `/ui` route with all components, color tokens, fonts, etc. simillar to https://www.radix-ui.com/themes/playground.
- Always use tailwindcss over regular css

## Svelte MCP

- For Svelte and SvelteKit work, use the official Svelte MCP server via `npx -y @sveltejs/mcp`. See https://svelte.dev/docs/ai/local-setup for client configuration.
- Start with `list-sections` to discover relevant documentation, then use `get-documentation` to read the sections needed for the task. Prefer current documentation over remembered framework APIs.
- Run `svelte-autofixer` on new or changed Svelte code, address its findings, and repeat until no actionable issues remain. This does not replace the site's checks or browser validation.
- Use `playground-link` for standalone examples only when useful or requested. Do not generate a playground link for code already written into the project.
- If the tools are unavailable, say so and consult the official documentation directly; do not claim MCP validation was performed.
