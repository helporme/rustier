## Current state
The plugin is before 0.1 release right now, many (all) features are not implemented yet and there are many bugs and issues to fix. The backwards compatibility does not exist, since there was no releases yet.

## Packages
There are currently two packages:
 - `rust-curse` - js wrapper and e2e
 - `rust-curse-native` - compiler

## Formatting
After editing any Rust file under `packages/rust-curse-native/`, run `cargo fmt` from that package directory before declaring the task done.
 
## Boundaries

It's a small section about code architecture.

### Functions with side effects
We distinguish the functions that cause side effects (also "impure") and functions that do not (also "pure"). Impure functions can call other impure functions or pure functions, but pure functions can *only* call other pure functions.

It naturally creates a code structure where "top-level" functions are causing side effects, but "leave" functions are usually not. 

As a rule, you want to have more pure functions and less impure functions in your module.

### Shell & Core
The top-level impure functions form a "shell" of a module. They usually read the input from external system, invoke pure functions ("core" of a module), and write output to the external system. Communication with the external system is a side effect, common examples of such:
 - Reading/writing file from the file system.
 - Getting current date time
 - Listening for keyboard input
 - Drawing on screen
 - Writing to the terminal
 - Generating random numbers
 - Logging
 - Sending web requests
 - Communicating with database

Core of the module is usually doing business operations.

### Unit and Integration tests
Unit tests are used primarily to test the core of the module (pure functions). Pure functions are naturally easy to test because of absence of side effect, hence you do not need to mock, stub or intercept anything. Unit tests can be exhaustive in branch coverage of a function they test.

Integration tests are used for shell of the module (impure functions). Integration tests should avoid exhaustive coverage, since integration tests usually become linearly slower over the time.

### No useless utility functions

Never introduce utility functions with less than 10 lines of code that are used only in one place. Always inline them.

### Allocation-light code generation

When composing generated source code, always use `frag!()` instead of `format!()` whenever possible to avoid intermediate allocations. Intead of vectors use `IntoIterator`, possibly with `+ Clone` on `IntoIter`. Only materialize an owned `String` when an API requires one.

### Fine grained errors

When a function returns a `Result`, its error type should represent all errors that can happen in that function and not more. Usually it means that you cannot use one error type for all functions in a module, and you should have more fine-grained ones.

### Useful docstrings

Add docstring comments to all non-trivial public functions, structs, traits and modules. Dostring should be a one sentence summary of what is the purpose of the function/struct/trait/module.


## Rust visibility

An item's effective visibility is already limited by its least-visible ancestor. When an item should be visible everywhere its containing module is visible, declare the item as `pub` instead of redundantly repeating the module's restriction. For example, use `pub struct Item` inside a `pub(crate) mod`, not `pub(crate) struct Item`.

### Do not violate the module boundaries

There is always obvious hierarchy between moduels. For example, rustdoc lowers to the ir, so it _knows_ about ir, but ir does not know about rustdoc, so when adding methods that convert something from rustdoc to ir, you must add a method under rustdoc module (like into_ir_* or lower_*), but NOT to the ir module (like from_ir_*).
