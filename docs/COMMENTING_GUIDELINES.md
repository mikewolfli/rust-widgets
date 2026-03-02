# Commenting Guidelines

This document defines the required English comment style for this repository.

## Scope

Applies to all Rust source files and C ABI headers/samples in this project.

## Required rules

1. Use English for all new comments.
2. Add comments for non-trivial logic (state transitions, platform bridges, mapping tables, fallback behavior).
3. Add doc comments for public APIs (`pub` items) when behavior is not obvious.
4. Keep comments synchronized with code changes in the same PR.
5. Remove stale comments instead of leaving misleading notes.

## Style rules

- Write concise, factual comments.
- Prefer present tense: "Returns...", "Maps...", "Queues...".
- Explain intent/behavior, not obvious syntax.
- Use consistent trigger terms: `clicked`, `value-changed`, `selection-changed`, `closed`, `unknown`.
- For cross-platform code, explicitly state normalization behavior.

## Good examples

- "Normalize native checkbox toggle into a click-like trigger event."
- "Queue menu trigger only for known menu item ids to avoid orphan events."
- "Convert UTF-8 text into zero-terminated UTF-16 for Win32 APIs."

## Avoid

- Redundant comments that repeat code literally.
- Vague comments like "do stuff" or "fix this".
- Long narrative comments without actionable meaning.

## Review checklist

- [ ] New/changed complex logic has English comments.
- [ ] Public API behavior is documented where needed.
- [ ] Comment terminology is consistent with project docs.
- [ ] No stale comments remain.
