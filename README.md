# Rustier

An experimental Rust-JavaScript interop tool for generating JS/TS bindings for any Rust crate, targeting WebAssembly or N-API.

- Supports wide range of Rust features like functions accepting references, struct and enums with field access, generic struct and enums, impls with generic constraints and many more. 
- Supports both N-API and WebAssembly, with identical API behavior.
- Generates TypeScript declarations with source maps.
- Can be used via a Vite plugin or a tsdown plugin, allowing you to place Rust files next to JS/TS files in your projects and import from them, see examples.
- Vite plugin supports **HMR** and **SSR**.
- Has great diagnostics!

## Installation

The package is not yet published to npm. Clone the repository locally, navigate to an example project under `examples/`, and run `pnpm run rebuild`, followed by `pnpm dev`, to try it out.

## Examples

### Hello world

<table><tbody><tr><td width="500px"> <code>./greet.rs</code> </td><td width="500px"> <code>./main.ts</code> </td></tr><tr>
<td valign="top">

```rust
pub fn greet(name: &str) -> String {
    format!("Hello {name}!")
}
```

</td><td valign="top">

```ts
import { greet } from './greet.rs'

console.log(greet('World'))
```

</td></tr></tbody></table>

### Structs and enums

<table><tbody><tr><td width="500px"> <code>./animals.rs</code> </td><td width="500px"> <code>./main.ts</code> </td></tr><tr>
<td valign="top">

```rust
pub struct Sheep {
    pub wool: u32,
}

pub enum Animal {
    Slime { size: u32 },
    Sheep(Sheep),
    Bug,
}

pub fn encounter() -> Animal {
    Animal::Sheep(Sheep { wool: 10 })
}
```

</td><td valign="top">

```ts
import { encounter } from './animals.rs'

const animal = encounter()

switch (animal.type) {
  case 'Slime':
    animal.size += 1
    console.log(animal.size)
    break
  case 'Sheep':
    console.log(animal[0].wool)
    break
  case 'Bug':
    console.log('A bug!')
    break
}
```

</td></tr></tbody></table>

### Borrowing and moving

When working with Rust structs and enums in JS, it's important to understand that each object is a **handle to a value on the Rust heap**, not a snapshot of the value. For example, a function accepting a struct by mutable reference modifies the same struct your JS code is using.

On the other hand, **moving** a struct or enum from JS to Rust clones it instead of transferring ownership of the original value. The original allocation stays alive while it is still referenced from JS.

<table><tbody><tr><td width="500px"> <code>./sheep.rs</code> </td><td width="500px"> <code>./main.ts</code> </td></tr><tr>
<td valign="top">

```rust
#[derive(Clone)]
pub struct Sheep {
    pub wool: u32,
}

pub fn adopt() -> Sheep {
    Sheep { wool: 10 }
}

pub fn is_fluffy(sheep: &Sheep) -> bool {
    sheep.wool >= 5
}

// Sheep implements Clone, so it can be
// passed by value from JS.
// Without Clone, this function is skipped
// with a diagnostic.
pub fn shear(mut sheep: Sheep) -> Sheep {
    sheep.wool = 0;

    sheep
}
```

</td><td valign="top">

```ts
import {
  adopt,
  is_fluffy,
  shear,
} from './sheep.rs'

// Keep this handle to work with the Sheep
// stored in Rust.
const sheep = adopt()

// A reference parameter avoids cloning
// the Sheep.
console.log(is_fluffy(sheep)) // true

// You can keep using sheep: this by-value
// call receives a clone.
const shorn = shear(sheep)
console.log(shorn.wool) // 0
console.log(sheep.wool) // 10 — still usable
```

</td></tr></tbody></table>

### Field access

When a field holds another struct or enum, **reading it gives you a reference, not a clone**. Changes made through that reference affect the value in the parent struct or enum.

Assigning to the field **clones the new value** and requires `Clone`. Any references to the old value remain usable.

In other words, struct or enum fields have very simillar semantics as normal js class fields, which makes a usage of it much more convinient.

<table><tbody><tr><td width="500px"> <code>./pasture.rs</code> </td><td width="500px"> <code>./main.ts</code> </td></tr><tr>
<td valign="top">

```rust
#[derive(Clone)]
pub struct Sheep {
    pub wool: u32,
}

impl Sheep {
    pub fn shear(&mut self) -> u32 {
        std::mem::take(&mut self.wool)
    }
}

pub struct Pasture {
    pub sheep: Sheep,
}

impl Pasture {
    pub fn new() -> Self {
        Self {
            sheep: Sheep { wool: 10 },
        }
    }
}
```

</td><td valign="top">

```ts
import { Pasture } from './pasture.rs'

const pasture = new Pasture()

// This refers to the Sheep inside pasture,
// so changes affect pasture.
const old_sheep = pasture.sheep
old_sheep.wool = 5
console.log(pasture.sheep.wool) // 5

// shorthand to `Sheep { wool: 10 }`
pasture.sheep = { wool: 10 }
console.log(pasture.sheep.wool) // 10

// This refers to the new field; old_sheep
// still holds the previous value.
const sheep = pasture.sheep
console.log(old_sheep.wool) // 5
console.log(sheep.wool)     // 10

// This mutates the Sheep in pasture.
console.log(sheep.shear())      // 10
console.log(pasture.sheep.wool) // 0
console.log(old_sheep.wool)     // 5
```

</td></tr></tbody></table>

### Generic types

Functions can accept generic structs and enums, but the function itself must not be generic: `fn count(items: Vec<String>)` is supported, but `fn count<T>(items: Vec<T>)` is not.

<table><tbody><tr><td width="500px"> <code>./inventory.rs</code> </td><td width="500px"> <code>./main.ts</code> </td></tr><tr>
<td valign="top">

```rust
pub fn item_count(
    items: Vec<String>,
) -> u32 {
    items.len() as u32
}

pub fn total_items(
    bags: Vec<Vec<String>>,
) -> u32 {
    bags.iter()
        .map(|bag| bag.len() as u32)
        .sum()
}
```

</td><td valign="top">

```ts
import {
  item_count,
  total_items,
} from './inventory.rs'
import { Vec } from '$rust/alloc/vec'

const Strings = Vec('String')
const items = new Strings()
items.push('healing potion')
items.push('dried mushrooms')
console.log(item_count(items)) // 2

const Bags = Vec(Strings)
const bags = new Bags()
bags.push(items)
console.log(total_items(bags)) // 2
```

</td></tr></tbody></table>

### Generic impls with trait bounds

<table><tbody><tr><td width="500px"> <code>./pouch.rs</code> </td><td width="500px"> <code>./main.ts</code> </td></tr><tr>
<td valign="top">

```rust
pub struct Pouch<T> {
    pub item: T,
}

impl<T> Pouch<T> {
    // For Pouch<String>, this becomes
    // new(item: String).
    pub fn new(item: T) -> Self {
        Self { item }
    }
}

impl<T> Pouch<T>
where
    T: Clone,
{
    // Available for Pouch<String> because
    // String implements Clone.
    pub fn duplicate(&self) -> Self {
        Self::new(self.item.clone())
    }
}

impl<T> Pouch<T>
where
    T: Default,
{
    // For Pouch<String>, returns a String
    // and leaves an empty string.
    pub fn take(&mut self) -> T {
        std::mem::take(&mut self.item)
    }
}

pub fn starter_pack() -> Pouch<String> {
    Pouch::new("healing potion".to_owned())
}
```

</td><td valign="top">

```ts
import {
  Pouch,
  starter_pack,
} from './pouch.rs'

const pouch = starter_pack()
const backup = pouch.duplicate()

console.log(pouch.take())   // healing potion
console.log(pouch.item)     // ""
console.log(backup.item)    // healing potion

const StringPouch = Pouch('String')
const snacks = new StringPouch(
  'dried mushrooms',
)
console.log(snacks.take()) // dried mushrooms
```

</td></tr></tbody></table>

### Construction from object literals

<table><tbody><tr><td width="500px"> <code>./point.rs</code> </td><td width="500px"> <code>./main.ts</code> </td></tr><tr>
<td valign="top">

```rust
#[derive(Clone)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn distance_to(
        &self,
        other: &Point,
    ) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;

        dx.hypot(dy)
    }
}
```

</td><td valign="top">

```ts
import { Point } from './point.rs'

const camp = Point.of({ x: 0, y: 0 })
const pond = Point.of({ x: 3, y: 4 })

console.log(camp.distance_to(pond)) // 5
console.log(
  camp.distance_to({ x: 3, y: 4 }),
) // 5
console.log(Point.of(camp) === camp) // true
```

</td></tr></tbody></table>

### Diagnostics

<table><tbody><tr><td width="500px"> <code>./summon.rs</code> </td><td width="500px"> Terminal </td></tr><tr>
<td valign="top">

```rust
pub async fn summon() -> String {
    "A slime appears!".to_owned()
}
```

</td><td valign="top">

```text
rustier::skipped_fn

  ⚠ the function "example::summon::summon"
    cannot be exported to JS
   ╭─[1:1]
 1 │ ╭─▶ pub async fn summon() -> String {
 2 │ │       "A slime appears!".to_owned()
 3 │ ╰─▶ }
   ╰────
  ├─▶ rustier::unsupported_async_fn
  │
  │     ⚠ async functions are not supported
  ╰─▶ rustier::unsupported_type

        ⚠ unsupported type:
          impl Future<Output = String>
```

</td></tr></tbody></table>

## Example projects

- [SvelteKit with vite](examples/sveltekit-example)
- [Library with tsdown](examples/library-example)

## When to use

Rustier aims to make using Rust inside JS/TS projects as easy as possible. Its goal is to reduce glue code, configuration, and common performance and memory-management pitfalls.

That said, Rustier works at a higher level than tools such as [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen) and [NAPI-RS](https://github.com/napi-rs/napi-rs), which it uses underneath. If you need finer control over the FFI boundary or features Rustier does not yet support, consider using these projects directly:

- [NAPI-RS](https://github.com/napi-rs/napi-rs) for Node.js addons, with direct access to JS values, async support, and callbacks. It also supports WASI/WebAssembly.
- [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen) for Rust-WebAssembly interop, including calling JS APIs from Rust.
- [Neon](https://github.com/neon-bindings/neon) for native Node.js addons with explicit bindings written using its Rust APIs.

Rustier is experimental; check the [limitations](#limitations) against your project's needs.

## Limitations

The following are not yet supported or remain limited. This list is not exhaustive:

- `i64`, `u64`, `i128`, `u128`, `isize`, `usize`, `f16`, and `f128`, as well as slices and non-unit tuples.
- Smart pointer support, including `Box<T>`, `Rc<T>`, and `Arc<T>`.
- Async functions and callbacks.
- Bindings for traits and trait impls. Trait bounds on inherent impls are supported.
- Mirroring Rust `use` statements and re-exports in generated JS modules.
- Passing arbitrary JS values into Rust, beyond the supported types and conversions.
- Idiomatic JS representations for C-like enums, unit structs, `Option`, `Result`, `Vec<T>`, `HashMap<K, V>`, and other collections. These currently use generated Rust-style bindings where supported.
- Rust code tree-shaking based on JS usage.

## How it works

rustier uses [rust-analyzer](https://rust-analyzer.github.io/) to analyze the Cargo project. It generates a separate Rust bridge crate for supported public APIs, along with JavaScript wrappers and TypeScript declarations, without modifying the original crate.

It consists of three parts:

1. [Build-tool plugins](packages/rustier/src) for Vite, Rolldown, and tsdown.
2. A [server](packages/rustier-native/src/server.rs) that handles plugin requests, such as `build` and `change`.
3. A [compiler](packages/rustier-native/src/compiler.rs) responsible for generating Rust and JS glue code and TypeScript declarations.

The most interesting part is the compiler, which is built on top of rust-analyzer's `hir` IR and uses the [Salsa](https://salsa-rs.github.io/salsa/) framework for incremental computation. It defines several additional [IRs](packages/rustier-native/src/compiler/ir):

1. [Rust IR](packages/rustier-native/src/compiler/ir/rust.rs) represents the supported subset of Rust features. It is lowered from rust-analyzer's `hir`, and most diagnostics are emitted during this step.
2. [Shim IR](packages/rustier-native/src/compiler/ir/shim.rs) is the central IR used by the compiler. Lowered from Rust IR, it represents JS concepts, such as classes, with references back to Rust IR entities, such as structs and enums.
3. [Facade](packages/rustier-native/src/compiler/ir/facade.rs) is lowered from Shim IR and used for JS code generation.
4. [Decals](packages/rustier-native/src/compiler/ir/decals.rs) is lowered from Shim IR and used for TypeScript declaration and source map generation.
5. [FFI](packages/rustier-native/src/compiler/ir/ffi.rs) represents the exact functions exposed across the Rust-JS boundary. It is intended to support future tree-shaking work.

The compiler also contains code generation machinery with its own concepts, such as [tethering](packages/rustier-native/src/compiler/codegen/shim/tethering.rs), [fragments](packages/rustier-native/src/compiler/codegen/fragment.rs), and [cast graphs](packages/rustier-native/src/compiler/codegen/shim/cast.rs).

The generated Rust bindings use [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen) for the WebAssembly target and [NAPI-RS](https://github.com/napi-rs/napi-rs) for native Node.js addons.

All of the above is covered by the following testing strategies, in order of importance:

1. [E2E tests](packages/rustier/tests/e2e) are the main corpus of tests, written in TypeScript.
2. [Property-based tests](packages/rustier-native/tests/proptests.rs) generate random Rust programs and check that the expected bindings are produced and the generated Rust code passes `cargo check`.
3. [Snapshot tests](packages/rustier-native/src/compiler/codegen) are mainly used for documentation, so developers can see what each part of the generated code looks like.
4. [UI tests](packages/rustier-native/tests/ui) prevent regressions for historical issues.

## AI

The project is primarily written by hand, with some AI help here and there. The notable exception is [source map code generation](packages/rustier-native/src/compiler/codegen/sourcemap.rs), which was mostly written by AI.

The main reason for writing it by hand is that even frontier LLMs struggle to maintain an extensible compiler architecture over time, and the initial attempts to implement this project using AI failed. This may change in the future, however.

## License

MIT
