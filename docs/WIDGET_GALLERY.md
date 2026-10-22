# Widget gallery

> This page is a **pointer**, not a list.

The authoritative gallery is [`control.md`](../control.md). It is generated from the
widget registry by `tools/generate_control_index.py` and verified by
`tools/check_svg_snapshots.sh` (one row per registered control, two appearances each).

## Why this page no longer carries a table

It used to claim "all available widgets" while listing 60 rows against a 188-name
registry, last edited 2026-06-09. A hand-maintained list drifts **silently**: the page
simply stops mentioning a control and nothing fails. `control.md` is a *view* of the
registry rather than a second copy of it, so it cannot drift; this path is kept so
existing links still resolve.

- Full gallery: [`control.md`](../control.md)
- Snapshot assets: `snapshots/svg/`
- Regenerate: `cargo run --example export_control_svgs`, then
  `python3 tools/generate_control_index.py`
