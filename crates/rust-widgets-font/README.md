# rust-widgets-font

The embedded font payloads used by [`rust_widgets`](https://crates.io/crates/rust_widgets)'
`fonts-*` features, kept in their own crate so the library stays under crates.io's 10 MiB package
ceiling.

Each module is one face:

| Module | Feature | Face |
|---|---|---|
| `latin` | `latin` | Open Sans, Latin-1 + ASCII + label punctuation |
| `arabic` | `arabic` | Noto Naskh Arabic, the Arabic block with joining forms |
| `cjk` | `cjk` | Noto Sans SC, kana / CJK punctuation / fullwidth / common Han |
| `cjk_shard_latin` … `cjk_shard_han` | `cjk-shard-*` | the same CJK face, partitioned by script |
| `emoji` | `emoji` | Noto Color Emoji (CBDT PNG bitmaps), 317 codepoints |

Every module exposes exactly one item, `pub const FONT: &[u8]` — the subset's bytes.

You do not normally depend on this crate directly: `rust_widgets` maps each of its `fonts-*`
features onto one of these. Nothing here is enabled by default; a consumer that asks for one face
does not compile the others.

## Licence

The crate's own source is MIT. The font data is **not**: every payload is a subset of a font under
the SIL Open Font License 1.1, and each subset is a Modified Version under that licence. See
[`NOTICE`](NOTICE) for the full per-face prose record, and the
[`rust_widgets` NOTICE](https://github.com/mikewolfli/rust-widgets/blob/main/NOTICE) for the
non-font material (the Unifont bitmap table and the icon outlines).
