# rust_widgets 0.5.0

Release date: 2026-03-02

## Highlights

- Basic widgets milestone is complete and stabilized:
  - Button
  - Label
  - LineEdit
  - CheckBox
  - RadioButton
  - ComboBox
  - SpinBox
  - Slider
  - ProgressBar

## Version

- Crate/package version is now `0.5.0`.

## Validation

This release was validated with the following checks (all passed):

- `bash tools/check_profiles.sh`
- `bash tools/check_abi.sh`
- `cargo test --lib`
- `bash tools/check_event_model_signal_first.sh`
- `bash tools/check_behavior_matrix.sh`
- `bash tools/check_visual_regression.sh`

## Reports

- Behavior matrix report: `target/qa/behavior_matrix_report.md`
- Visual regression report: `target/qa/visual_regression_report.md`

## Suggested Tag Commands

```bash
git tag -a v0.5.0 -m "rust_widgets 0.5.0"
git push origin v0.5.0
```

## Optional: Create GitHub Release

Use this file content as the release description body for tag `v0.5.0`.