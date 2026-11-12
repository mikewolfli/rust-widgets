# Contributing to rust_widgets

Thank you for your interest in contributing.

## Development setup

1. Install Rust stable toolchain.
2. Clone the repository.
3. Run the applicable local gates:

```bash
# The device profiles are mutually exclusive, so name one explicitly.
# Do NOT use `--all-features`: it enables `desktop` and `mini` together, and
# `mini` switches the crate to `no_std`, so that combination cannot compile.
cargo check --all-targets --no-default-features --features "desktop,code-editor,markdown-editor,builtin-languages"
cargo fmt --all -- --check
cargo clippy --all-targets --no-default-features --features "desktop,code-editor,markdown-editor,builtin-languages" -- -D warnings
cargo test --no-default-features --features "desktop,code-editor,markdown-editor,builtin-languages" -q
cargo test --no-default-features --features "desktop,icons" -q

# Remaining CI test profiles:
for profile in full embedded mini tablet mobile; do
  cargo test --no-default-features --features "$profile" -q
done

# Build-only profile checks, including the stripped profiles:
for profile in desktop tablet mobile mini embedded; do
  cargo check --no-default-features --features "$profile"
done

# Source/API/platform contract gates; this is separate from the Rust test matrix.
bash tools/run_all_gates.sh
```

`cargo test --no-default-features --features "desktop,code-editor,markdown-editor,builtin-languages"` is the main desktop test
suite, not a test of every possible feature combination. CI additionally tests
the opt-in icon data, `tablet` and `mobile`, plus `full`, `embedded`, and `mini`
in the feature matrix. The desktop/default test configuration is run once.
Native GTK, Apple, and Android tests run in their platform-specific CI jobs.
Code coverage is collected separately by CI's `cargo llvm-cov` job. Do not
replace these configurations with `--all-features`: the device profiles are
mutually exclusive. `run_all_gates.sh` runs the source and contract gates; it
does not replace the Cargo test matrix, and host-limited gates may report a skip.

## Dependency and version policy

This repository is **not** a Cargo workspace. The root package, each crate under
`crates/` (five), each demo under `demo/` (five) and the three probe packages
under `tools/` resolve independently, so a working tree can contain fourteen
package-level `Cargo.lock` files (a fifteenth can appear under a build `target/`
directory, which is output rather than an input). A shared transitive dependency can
therefore resolve to different versions in different packages.

- A dependency bump must be applied to every `Cargo.toml` that names it; bumping
  only the root leaves the companion and demo packages on the old resolution.
- When auditing dependencies, regenerate **every** lockfile, not just the root one:

  ```bash
  cargo update --manifest-path Cargo.toml
  for manifest in crates/*/Cargo.toml demo/*/Cargo.toml tools/*/Cargo.toml; do
    [ -f "$manifest" ] && cargo update --manifest-path "$manifest"
  done
  ```

- Compare the resolved version of a shared dependency across packages with
  `cargo tree --manifest-path <manifest> -i <crate>` before claiming a uniform
  dependency graph.

Companion crates must carry the same version as the root package; the release
checks enforce that. The absence of a workspace is intentional (the nested packages
are excluded from the published tarball), so do not "fix" it by adding one.

## Branch and commit

- Create a feature branch from `main`.
- Keep commits focused and atomic.
- Use clear commit messages.

## Pull request checklist

- [ ] Code compiles with `cargo check --all-targets --no-default-features --features "desktop,code-editor,markdown-editor,builtin-languages"`.
- [ ] Passes `cargo clippy --all-targets --no-default-features --features "desktop,code-editor,markdown-editor,builtin-languages" -- -D warnings`.
- [ ] Tests pass with `cargo test --no-default-features --features "desktop,code-editor,markdown-editor,builtin-languages" -q`.
- [ ] Icon-data tests pass with `cargo test --no-default-features --features "desktop,icons" -q`.
- [ ] Tests pass for the applicable `tablet`, `mobile`, and `embedded` profiles.
- [ ] All five device profiles still build (`desktop`, `tablet`, `mobile`, `mini`, `embedded`).
- [ ] Formatting passes `cargo fmt --all -- --check`.
- [ ] Applicable source and contract gates pass with `bash tools/run_all_gates.sh`.
- [ ] Documentation is updated when behavior changes.
- [ ] No unrelated refactoring.
- [ ] New/changed logic includes English comments following `docs/COMMENTING_GUIDELINES.md`.
- [ ] Controls touched by the PR are implemented end-to-end for supported backends (creation, state sync, events, and data path), not visual-only placeholders.
- [ ] Missing control capability is explicit (`unsupported`/`0`) with docs/TODO tracking; no silent fallback substitution to unrelated controls.

## Style guidelines

- Prefer small, focused changes.
- Keep APIs backward compatible when possible.
- Follow existing module and naming conventions.
- Add clear English comments/doc comments for new or modified logic (especially non-trivial control flow, platform bridges, and exported ABI functions).
- Keep comments synchronized with code changes; outdated comments should be updated in the same PR.
- Follow detailed comment style rules in `docs/COMMENTING_GUIDELINES.md`.
- Do not ship "minimum" control implementations that only render a shell; complete runtime behavior is required on supported backends.

## Reporting issues

Use the issue templates and provide:

- Reproduction steps
- Expected behavior
- Actual behavior
- Environment (OS, Rust version)
