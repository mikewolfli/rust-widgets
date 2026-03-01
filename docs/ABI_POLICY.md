# ABI Evolution Policy

This document defines how `rust_widgets` evolves its C ABI while preserving compatibility.

## Scope

- Applies to exported `extern "C"` symbols in `src/bindings/mod.rs`.
- Applies to generated/maintained C headers:
  - `examples/rust_widgets.generated.h`
  - `examples/rust_widgets.h`
- Applies to release gates in `tools/check_abi.sh` and CI `validation-gates`.

## Current Baseline

- Current ABI version: `5` (`rust_widgets_bindings_api_version`).
- Existing symbols at ABI version `5` are backward compatibility commitments.

## Compatibility Rules

### Rule 1: Additive changes are preferred

The following are ABI-compatible and do **not** require breaking changes:

- Adding new exported functions.
- Adding new bit flags to masks where old bits keep their original meaning.
- Extending behavior behind existing functions without changing parameter/return types.

### Rule 2: Breaking changes require an ABI version bump

The following are ABI-breaking and require ABI version increment and release note callout:

- Removing an exported symbol.
- Renaming an exported symbol.
- Changing function signature (parameter count/type/order, return type).
- Reinterpreting existing bit meanings incompatibly.

### Rule 3: Header parity is mandatory

Any C ABI export change must update header output and pass `tools/check_abi.sh`.

### Rule 4: Reserved endpoints policy

`rust_widgets_python_reserved`, `rust_widgets_cpp_reserved`, and `rust_widgets_java_reserved`
may evolve to real implementations, but must remain ABI-safe (additive path preferred).

## ABI Compatibility Matrix

| Change Type | ABI Compatible | Requires ABI Version Bump | Notes |
|---|---:|---:|---|
| Add new function | Yes | No | Keep old symbols unchanged |
| Remove/rename function | No | Yes | Breaking for existing clients |
| Change function signature | No | Yes | Breaking at link/runtime boundary |
| Add new bit flag | Yes | No | Preserve old bit semantics |
| Reassign existing bit meaning | No | Yes | Breaking behavior |
| Internal implementation refactor | Yes | No | If public ABI remains identical |

## Release Gate Checklist (ABI)

Before release:

1. Run `tools/check_abi.sh`.
2. Ensure generated header has no drift.
3. Confirm `rust_widgets_bindings_api_version` remains correct.
4. If any breaking change exists, bump ABI version and document in `CHANGELOG.md`.

## Migration Guidance

When introducing a replacement for existing behavior:

- Add a new symbol first.
- Keep old symbol available for at least one release cycle.
- Document deprecation path in `CHANGELOG.md` and docs.
