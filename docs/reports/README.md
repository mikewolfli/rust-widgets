# Archived reports

> ⚠️ **历史快照（archive）**：本目录下的报告是某一轮工作的取证与度量记录，不是当前状态的
> 声明。带「100%」「已完成」字样的报告只对当时那一轮的范围成立；当前状态以 `Cargo.toml`、
> `CHANGELOG.md` 与 CI 输出为准。

Reports are retained for their measurements and decision trail, not as a live status page.

## Index

| File | What it records |
|---|---|
| [`CHANGELOG.md`](CHANGELOG.md) | Canonical changelog (byte-identical to the repository root copy, enforced by `tools/check_changelog_sync.sh`) |
| [`COMPLETION_NOTICE.md`](COMPLETION_NOTICE.md) | A dated completion statement; scope is that round only |
| [`PROJECT_COMPLETION_REPORT.md`](PROJECT_COMPLETION_REPORT.md) | The larger completion report behind `COMPLETION_NOTICE.md` |
| [`RELEASE_NOTES_0.5.0.md`](RELEASE_NOTES_0.5.0.md) | Release notes for 0.5.0 |
| [`FILE_SIZES.md`](FILE_SIZES.md) | Per-file line counts used by the source-structure rule (#9) |
| [`MODULE_RESPONSIBILITIES.md`](MODULE_RESPONSIBILITIES.md) | Which module owns which layer |
| [`miri_audit_report.md`](miri_audit_report.md) | Miri audit of the unsafe surface |
| [`partial_redraw_measurement.md`](partial_redraw_measurement.md) | Damage/partial-redraw measurements |
| [`scrollable_census.md`](scrollable_census.md) | Row-unit scroll census (RW-58) |
| [`setter_census.md`](setter_census.md) | Setter/command census (RW-44) |
