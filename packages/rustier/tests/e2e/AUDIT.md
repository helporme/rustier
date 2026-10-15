# Binding e2e audit

## Reproduce

From the repository root:

```sh
pnpm run test/e2e
```

The added bug reproductions are ordinary tests, not skipped tests or expected
failures. The suite is deliberately red until the two issues below are fixed.
No compiler or runtime implementation was changed as part of this audit.

## Confirmed failures

### Owned clones are sensitive to parameter order

`src/clone-and-borrow/test.ts` includes a passing control and two failing cases:

- `add_owned_first(counter, counter)` passes. Its Rust signature puts the owned
  `Counter` before `&mut Counter`.
- `add_owned_last(counter, counter)` throws. Its signature reverses those
  parameters.
- `counter.add(counter)` throws for the same reason.

Both Node and Wasm report:

```text
cannot access a tethered value while it is mutably borrowed
```

Argument conversions are nested in parameter order. A mutable reference stays
borrowed while the subsequent owned argument tries to clone that allocation.
Preparing owned clones before holding reference borrows would remove this
order dependency without allowing aliased mutable references.

### A field named constructor produces invalid TypeScript

`src/js-names/test.rs` declares a public `constructor: i32` field. Its runtime
test passes, but the generated declaration contains:

```ts
public get constructor(): number;
public set constructor(value: number);
```

TypeScript rejects both accessors with `Class constructor may not be an
accessor`. The generator already uses computed names for instance methods
named `constructor`, but not for field accessors.

Vitest reports these as unhandled source errors, not failed `expectTypeOf`
assertions. Its `Type Errors: no errors` summary does not mean the declarations
passed: the separate source errors make the command fail.

## Coverage added

| Area | Bug-oriented checks |
| --- | --- |
| Generic bindings | Specialization identity, primitive/class/nested keys, argument order, constructor substitution, method-level and impl-level trait bounds, coercion availability, and enum narrowing |
| Invalid generic input | Wrong arity, unsupported keys, templates instead of concrete class keys, and wrong-specialization arguments |
| Standard-library generics | Cross-module `Vec`, `Option`, and `Result` identity; both enum branches; nested vector cloning; mutation and `pop` |
| Ownership and borrowing | Same-root shared references, clone/reference ordering, receiver aliasing, borrow release after failed conversion and alias errors |
| Resource cleanup | Deterministic `Drop` counts for owned clones when later function arguments or coercion fields fail conversion |
| Projected fields | Multi-level detachment and assigning a projected child back into its own parent |
| Numeric boundaries | Fractions, NaN, infinities, signed zero, f32 overflow/underflow, Unicode scalar boundaries, and the previously unimplemented char/u32 test |
| Strings | Empty strings, embedded NULs, large Unicode allocations, and subsequent calls |
| JavaScript names | Raw identifiers, reserved parameter names, `constructor` and `__proto__` fields, and keyword method names |

The final audit run discovered 672 test cases across 126 project/test-file
combinations: 668 passed, four failed (the two clone-order cases on both
targets), and four additional source errors came from the invalid constructor
accessors. No `.skip` or `.todo` cases remain in this binding suite.

## Important remaining gaps

- Garbage-collection finalization, children outliving collected parent wrappers,
  and repeated allocation/release need an isolated GC-enabled test process.
  The deterministic `Drop` tests above cover error-path clones, not finalizers.
- Rust panic behavior and post-panic usability need subprocess isolation so a
  native abort or Wasm trap cannot terminate unrelated tests.
- JavaScript keyword names on free exports and classes need compile/load tests
  in addition to the property and parameter cases added here.
- Vite HMR, SSR, production bundles, and target/profile switching need a separate
  integration harness. The existing e2e README deliberately excludes plugin
  lifecycle behavior from this binding suite.
