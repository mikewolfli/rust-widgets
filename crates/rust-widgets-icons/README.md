# rust-widgets-icons

Material Symbols outlined icon geometry for [`rust_widgets`](https://crates.io/crates/rust_widgets).

The crate ships the `IconName` enum and the geometry behind it — 4,323 icons: the 75 tokens
`rust_widgets` names itself, plus the whole Material Symbols catalogue.

- `IconName` — the generated name table (`as_str`, `from_name`, `ALL`, `CURATED_TOKENS`).
- `icon_data()` — the SVG path `d` table, available with the `data` feature (on by default in
  the device profiles through `rust_widgets`'s own `icons` feature).
- `icon_fallback()` — the always-compiled flattened polygons a build without `data` draws. The
  table is dense (one entry per token) but only the first `FALLBACK_GEOMETRY_ENTRIES` carry
  geometry: those are the 75 curated tokens, and the catalogue's entries are empty so a
  `mini`/`embedded` build does not carry ~12 MB of polygons it never draws.
- `UPSTREAM_ALIASES` — the `(alias, canonical)` pairs for tokens Material Symbols draws with one
  picture (`star` / `grade` / `star_outline`), so a host can group an icon picker and a test can
  tell upstream's aliasing from a table that draws two unrelated names as one.

The geometry is a derivative of Material Symbols (Google LLC, Apache-2.0). 146 of the 4,323
outlines are the release tree's rather than the pinned commit's — 73 names are absent from the
pinned tree and 73 exist there only in upstream's legacy 24-unit encoding. See `NOTICE` and
`LICENSE-APACHE`.

## Features

- `data` (off by default) — compile the SVG path table. Without it the crate still exposes the
  name table, the fallback polygons and the upstream-alias relation.
