# QA Harness

This document describes the cross-profile behavior matrix and visual regression harness.

## Behavior Matrix

Run:

```bash
tools/check_behavior_matrix.sh
```

What it validates:

- Capability contract consistency across `default`, `embedded`, and `full,mobile-api`.
- Menu trigger event parity.
- Typed widget trigger parity.
- Embedded C/Python/Java demo output schema parity (`KEY=VALUE` + key order).

Output report:

- `target/qa/behavior_matrix_report.md`

## Visual Regression

Run:

```bash
tools/check_visual_regression.sh
```

What it validates:

- Deterministic SVG snapshot hash for line chart rendering.
- Deterministic SVG snapshot hash for bar chart rendering.

Output report:

- `target/qa/visual_regression_report.md`

## Embedded Demo Schema

Run:

```bash
tools/check_embedded_demo_schema.sh
```

Note: this check is also included inside `tools/check_behavior_matrix.sh`.

What it validates:

- C/Python/Java embedded demo outputs use the same `KEY=VALUE` schema.
- Output key order is identical across all three language demos.

Scope:

- `examples/c_abi_embedded_engine_demo.c`
- `examples/python/demo_embedded_engine.py`
- `examples/java/RustWidgetsEmbeddedEngineDemo.java`

## Full QA Pass

```bash
tools/check_behavior_matrix.sh && tools/check_visual_regression.sh
```

Use this command in CI or release gates to detect behavior and rendering regressions.
