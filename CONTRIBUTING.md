# Contributing to rust_widgets

Thank you for your interest in contributing.

## Development setup

1. Install Rust stable toolchain.
2. Clone the repository.
3. Run checks:

```bash
cargo check
cargo check --examples
```

## Branch and commit

- Create a feature branch from `main`.
- Keep commits focused and atomic.
- Use clear commit messages.

## Pull request checklist

- [ ] Code compiles with `cargo check`.
- [ ] Examples compile with `cargo check --examples`.
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
