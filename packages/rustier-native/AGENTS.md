# Rustier agent guide

## Documentation

- Do not add tests unless requested. You may add temporary tests during implementation, but remove them before finishing and notify the developer in a test coverage gap.
- Do not add comments unless requested, su

## Code style
- Do not introduce utility functions under 10 lines if they are used a few times. Inline them instead.
- By default assume that all code is private and there are no breaking changes.
- Avoid adding assertions in non-test code, espesially debug-only assertions. Better structure the code around `unreachable!()` macro.
- Do not defensively handle states excluded by the program’s invariants. If such a state occurs, fail immediately (using `unreachable!()` where appropriate) so the bug remains visible and fixable; do not silently deduplicate, default, or otherwise recover. Reserve validation and recovery for external inputs, explicitly unsupported representations, and behavior required by domain semantics.
- Add blank lines between logical groups of statements in generated and edited Rust code, especially between setup, checks, traversal, and final returns.
- When a function returns `Result`, its error type should cover exactly the errors that function can produce. Prefer fine-grained error types over a single module-wide error type.

## Codegen
- When generating source code, prefer `frag!()` over `format!()` whenever possible to avoid intermediate allocations. Prefer `IntoIterator` over vectors where possible, adding `+ Clone` to `IntoIter` when needed. Only materialize an owned `String` when required by an API.
- In `frag!`, write source tokens directly; use `{expr}` for interpolation, `{{ ... }}` for braces, `~` for spaces, and `@` for newlines.

## Module ordering

Order Rust module members roughly as follows:

1. Module documentation and attributes
2. `use` imports
3. Constants and statics
4. Type-centric items:
   - Type definition (`struct`, `enum`, `trait`, etc.)
   - Inherent `impl` blocks
   - Trait `impl` blocks
   - Public free functions
   - Private helper functions
   - Other related items
5. Tests
