## Rust

- Do not add tests unless requested. You may add temporary tests during implementation, but remove them before finishing.
- Do not introduce utility functions under 10 lines if they are used only once. Inline them instead.
- All code is private, there are no breaking changes.
- Avoid adding assertions.
- Do not add comments unless requested.
- When a function returns `Result`, its error type should cover exactly the errors that function can produce. Prefer fine-grained error types over a single module-wide error type.
- When generating source code, prefer `frag!()` over `format!()` whenever possible to avoid intermediate allocations. Prefer `IntoIterator` over vectors where possible, adding `+ Clone` to `IntoIter` when needed. Only materialize an owned `String` when required by an API.

### Module ordering

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
