# rust-widgets — Node.js binding

The binding loads the Rust `cdylib` over [`koffi`](https://www.npmjs.com/package/koffi) and
wraps the `rw_*` C ABI in a JavaScript API. It is `"private": true` in `package.json`: it ships with
this repository rather than to npm.

## Toolchain constraint: none (measured, not inferred)

The binding loads the Rust `cdylib` over [`koffi`](https://www.npmjs.com/package/koffi), which ships
**prebuilt binaries**: `npm install` needs no compiler, no `node-gyp` and no `node-addon-api`, and the
probe passes on every Node the project tests:

| Node | `npm install` | `node verify_icon_contract.js` |
|---|---|---|
| 16.20.2 | ✅ (2 packages, no build step) | ✅ 17/17 checks |
| 18.20.8 | ✅ | ✅ 17/17 checks |
| 20.20.2 | ✅ | ✅ 17/17 checks |
| 22.23.3 | ✅ | ✅ 17/17 checks |
| 24.15.0 | ✅ | ✅ 17/17 checks |

# Why koffi rather than the previous ffi-napi

The binding used `ffi-napi@4.0.3`, which is unbuildable on Node 18+: its own `src/ffi.cc` stores the
const-env `get-uv-event-loop` pointer in a non-const slot.

```text
src/ffi.cc: error: invalid conversion from 'napi_status (*)(node_api_basic_env, uv_loop_s**)'
  {aka 'napi_status (*)(const napi_env__*, uv_loop_s**)'} to 'get_uv_event_loop_fn'
  {aka 'napi_status (*)(napi_env, uv_loop_s**)'} [-fpermissive]
```

Two things were measured before switching, and each rules out a cheaper fix:

* **Overriding `node-addon-api` does not help.** `ffi-napi@4.0.3` and `ref-napi@3.0.3` both pin
  `node-addon-api@^3`; raising it to `^8.9.2` does fix the `napi_finalize` half of the problem, but
  `node-gyp --build-from-source` still stops at the `get_uv_event_loop_fn` conversion above, because
  that conversion is in `ffi-napi`'s own translation unit.
* **There is no maintained drop-in fork.** `@lwahonen/ffi-napi@4.0.12` (published 2023-03) fails with
  the identical error, so no npm `overrides` / alias entry can make Node 18+ work.

The port was bounded: the declarations were already a **single data table** (`name: [returns, [args]]`)
consumed by one `ffi.Library` call, and every call site went through `this._lib.rw_*`, so koffi only
had to replace that one construction plus the type aliases and four helpers. Two koffi behaviours
decided the shape of the port and are worth knowing before editing this file:

* **`char *` is declared opaque** (`koffi.opaque("rw_string")`). A plain `koffi.pointer("char")` is
  auto-converted to a JS string on the way out, which loses the pointer `rw_free_string` needs — a
  leak per call. With the opaque type the out-slot decodes to a pointer the free accepts.
* **A `uint64` is a BigInt, and a numeric argument must not be a String.** koffi returns BigInt for
  64-bit values and rejects a decimal String for a numeric slot (`TypeError: Unexpected String value,
  expected number`), so the old `int64Argument` conversion became the identity — passing the BigInt is
  both exact and required.

## Build and run

```bash
# 1. Build the library this binding loads (from the repository root).
cargo build --no-default-features --features desktop      # -> target/debug/librust_widgets.so

# 2. Install the binding's dependencies. koffi ships prebuilt binaries, so this needs no compiler
#    and no Node version pin (measured: 16.20.2 through 24.15.0).
cd bindings/nodejs && npm install

# 3. Point the binding at the library and run something.
RUST_WIDGETS_LIB_PATH=../../target/debug/librust_widgets.so node example.js
```

`RUST_WIDGETS_LIB_PATH` is only needed when the library is not in one of the default search locations
(`./target/release/librust_widgets.so`, `./target/debug/librust_widgets.so`, …) relative to the
current directory.

## Four defects found by running this binding

Recorded because they are invisible to inspection and both were reproduced deliberately:

1. **A BigInt in a 64-bit argument slot is rejected by `ref-napi`** — `writeInt64` / `writeUInt64:
   Number/String 64-bit value required`. The encoder produced BigInts, so every property write
   threw. Ids are BigInts too, so the binding converts at the single call boundary
   (`withWireArguments`), using a Number when the value is exact and a decimal String beyond 2^53.
2. **An unused `require("ref-struct-napi")` broke the binding outright.** The module was imported and
   never used, but loading it pulls a second, older `ref-napi` into the process: restoring *only*
   that import (with every other fix in place) made the probe abort with a V8 core dump. It is gone
   from `index.js` and from `package.json`'s dependencies.

## Verifying the icon contract

`verify_icon_contract.js` drives the typed `icon_source` property through the real ABI — the JS
descriptor encoder/decoder, the C ABI's value kinds, and the Rust property layer — so a JS-side
encoding bug cannot hide behind the Rust and Python suites:

```bash
cd bindings/nodejs
RUST_WIDGETS_LIB_PATH=../../target/debug/librust_widgets.so node verify_icon_contract.js
# NODE-ICON-PROBE: PASS
```

It asserts that a built-in descriptor round-trips field for field, that an explicit `null` clear is
distinguishable from "never set", that a `resource` descriptor is **refused** without an apply
context (rather than becoming an assumed file path), and that a malformed descriptor is refused
before it reaches the ABI.

## Ids are `BigInt`

Widget ids come back from the ABI as `BigInt`, not `Number`:

* this library's `ObjectId`s sit above `Number.MAX_SAFE_INTEGER` — measured:
  `rw_create_window -> 6000285942072475648`, the next control `-> 6000285942072475649`;
* doubles are spaced 1024 apart up there, so `Number(windowId)` and `Number(buttonId)` were the
  **same** value, and every id-based call addressed whichever widget the rounded id landed on
  (`widgetPropertyNames(buttonId)` answered with the window's properties).

So compare against a BigInt (`id === 0n`, not `id === 0`), and otherwise treat an id as opaque. koffi
accepts a BigInt for a 64-bit argument and keeps every bit, so an id can be passed straight back into
any method without conversion.
