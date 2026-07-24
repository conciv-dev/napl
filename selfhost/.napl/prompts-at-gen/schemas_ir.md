# The intermediate-representation (IR) schema

This module defines the data types for a module's contract-level intermediate
representation and a validator that parses an arbitrary JSON value into that
shape, enforcing a couple of semantic rules beyond what the type system checks.
It is pure: no I/O and no dependencies on other project modules. Bring in `serde`
and `serde_json` (the IR's free-form `given`/`expect` fields are held as
`serde_json::Value`).

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `schemas_ir/`: `schemas_ir/Cargo.toml` (package
name `schemas_ir`) and `schemas_ir/src/lib.rs`. Touch nothing outside
`schemas_ir/`. Ensure `cargo test` passes from the workspace root before
finishing.

## The types

All of these derive `serde::Deserialize` and support equality comparison:

- `IrType` with public string fields `name` and `description`.
- `IrFunction` with public string fields `name`, `signature`, and `behavior`.
- `IrTest` with a public string field `name`, and two public fields `given` and
  `expect`, each a `serde_json::Value`. When `given` or `expect` is omitted from
  the input it defaults to an **empty JSON object** (`{}`), not null.
- `Ir` with a public string field `module`, and four list fields that each
  default to empty when omitted: `deps` (a list of strings), `types` (a list of
  `IrType`), `functions` (a list of `IrFunction`), and `tests` (a list of
  `IrTest`).

## The validator — `validate_ir(value)`

`validate_ir` takes a `serde_json::Value` and returns the parsed `Ir` on success
or an error on failure (an error value, never a panic). It:

1. Deserializes the value into an `Ir`. This step alone rejects inputs whose
   fields have the wrong type or shape — for example a function entry that is
   missing its `signature` and `behavior` (both are required, with no default),
   so `{ "name": "f" }` as a function is invalid.
2. Requires `module` to be a non-empty string. An IR with no `module` field at
   all, or with `module` set to the empty string `""`, is rejected.
3. Requires every test case's `given` and `expect` to be **either a JSON object
   or a JSON string**. Any other JSON type there (number, boolean, array, null)
   is rejected.

## Worked behavior to reproduce exactly

- `{ "module": "greeting", "functions": [{ "name": "greet", "signature":
  "greet(name): string", "behavior": "returns Hello" }] }` validates: the result
  has `module` = `greeting`, empty `deps`/`types`/`tests`, and its one function's
  `name` is `greet`.
- `{ "functions": [] }` (no module) is rejected; `{ "module": "", "functions": []
  }` (empty module) is rejected; `{ "module": "x", "functions": [{ "name": "f" }]
  }` (incomplete function) is rejected.
- `{ "module": "m", "tests": [{ "name": "a", "given": { "x": 1 }, "expect": "ok"
  }] }` validates, and the first test's `expect` equals the JSON string `"ok"`.
- `{ "module": "m", "tests": [{ "name": "a" }] }` validates, and the first test's
  `given` equals the empty JSON object `{}`.
