# E2E Tests

This package contains e2e tests for rustier, mostly for the bindings codegen layer. The goal is to make sure that Rust entities (functions, structs, enums, etc.) are correctly exposed to JS and behave as expected, both for Wasm and Node.js APIs.

This package deliberately does not set as a goal to test the behavior of the Vite plugin itself, so configs, HMR experience, project building or stuff like that. It will be tested separately in other e2e packages in the future.

## Contributing

All tests are grouped into category folders and stored under `src/` in a big flat list; new categories are added as needed. 

All `test.ts` files by default are tested against both Node.js and WebAssembly targets; use `node.ts` and `wasm.ts` for platform-specific behavior. Usually all visible behavior is aimed to be identical across both targets, so platform-specific tests are usually added to represent known errors.

Some general rules:
- Keep failure scenarios in dedicated `*-throws` directories so it's clear which scenarios are expected to fail.
- When adding new tests, try to use realistic names and scenarios that reflect the real-world usage of the code. So when testing enums, it's better to use `Color::Rgb` rather than `EnumTest::Foo`. 
- Write test descriptions driven by examples, so "Shape::Circle passed twice as a mutable reference throws" is better than "Struct-shaped enum variant passed twice as a mutable reference throws".
