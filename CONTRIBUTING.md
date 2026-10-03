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
cargo check --all-targets --no-default-features --features desktop
cargo fmt --all -- --check
cargo clippy --all-targets --no-default-features --features desktop -- -D warnings
cargo test --no-default-features --features desktop -q
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

`cargo test --no-default-features --features desktop` is the main desktop test
suite, not a test of every possible feature combination. CI additionally tests
the opt-in icon data, `tablet` and `mobile`, plus `full`, `embedded`, and `mini`
in the feature matrix. The desktop/default test configuration is run once.
Native GTK, Apple, and Android tests run in their platform-specific CI jobs.
Code coverage is collected separately by CI's `cargo llvm-cov` job. Do not
replace these configurations with `--all-features`: the device profiles are
mutually exclusive. `run_all_gates.sh` runs the source and contract gates; it
does not replace the Cargo test matrix, and host-limited gates may report a skip.

## Branch and commit

- Create a feature branch from `main`.
- Keep commits focused and atomic.
- Use clear commit messages.

## Pull request checklist

- [ ] Code compiles with `cargo check --all-targets --no-default-features --features desktop`.
- [ ] Passes `cargo clippy --all-targets --no-default-features --features desktop -- -D warnings`.
- [ ] Tests pass with `cargo test --no-default-features --features desktop -q`.
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
