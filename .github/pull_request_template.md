## Summary

Describe what this PR changes.

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Refactor
- [ ] Documentation

## Validation

- [ ] `cargo check --all-targets --no-default-features --features desktop`
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --no-default-features --features desktop -- -D warnings`
- [ ] `cargo test --no-default-features --features desktop -q`
- [ ] `cargo test --no-default-features --features "desktop,icons" -q` (when icon data/rendering is affected)
- [ ] Profile tests/build checks and `bash tools/run_all_gates.sh` as applicable

## Checklist

- [ ] I updated related docs.
- [ ] I kept changes focused and backward compatible where possible.
