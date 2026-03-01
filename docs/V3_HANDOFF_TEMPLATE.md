# v3 Delivery Checklist & Handoff Template

Use this template before tagging or publishing a new crate version.

## 1) Release Metadata

- [ ] `Cargo.toml` version updated
- [ ] crates.io metadata validated (`rust-version`, repository, docs, homepage, keywords, categories)
- [ ] release notes cut in `CHANGELOG.md`

## 2) Validation Gates

```bash
tools/check_profiles.sh
tools/check_abi.sh
tools/smoke_demos.sh
```

- [ ] profile matrix gate passed
- [ ] ABI gate passed
- [ ] demo smoke gate passed
- [ ] ABI policy reviewed (`docs/ABI_POLICY.md`) and compatibility impact classified

## 3) CI Verification

- [ ] GitHub Actions `check` job green
- [ ] GitHub Actions `validation-gates` job green

## 4) Publish Readiness

```bash
cargo publish --dry-run
```

- [ ] dry-run passed
- [ ] package size and included files reviewed
- [ ] if ABI-breaking change exists: ABI version bumped + changelog section added

## 5) Publish

```bash
cargo publish
```

- [ ] crates.io package visible
- [ ] docs.rs build available

## 6) Handoff Notes

- Release version:
- Date:
- Commit SHA:
- CI run URL:
- Crates.io URL:
- Docs.rs URL:
- Known limitations / follow-ups:
