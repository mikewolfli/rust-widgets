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

## Style guidelines

- Prefer small, focused changes.
- Keep APIs backward compatible when possible.
- Follow existing module and naming conventions.
- Add clear English comments/doc comments for new or modified logic (especially non-trivial control flow, platform bridges, and exported ABI functions).
- Keep comments synchronized with code changes; outdated comments should be updated in the same PR.
- Follow detailed comment style rules in `docs/COMMENTING_GUIDELINES.md`.

## Reporting issues

Use the issue templates and provide:

- Reproduction steps
- Expected behavior
- Actual behavior
- Environment (OS, Rust version)
