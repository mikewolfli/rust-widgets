# Commenting Guidelines

## Purpose
Comments should explain intent, invariants, and non-obvious design decisions.
Do not repeat code that is already clear from names and structure.

## Required Rules
- Write all code comments in English.
- Keep comments accurate and update them with code changes.
- Prefer API docs (`///`) for public items and inline comments (`//`) only for complex logic.
- Explain why a branch exists when behavior is surprising or constrained by platform/runtime details.
- Document safety requirements for every `unsafe` function or block.

## Recommended Style
- Use short, direct sentences.
- Put comments above the code they describe.
- Use bullet lists for multi-condition behavior.
- Include edge-case intent when a branch handles unusual input.

## Avoid
- Placeholder comments such as TODO/FIXME without actionable context.
- Comments that mirror trivial operations.
- Stale comments that no longer match behavior.

## Examples
Good:
```rust
// Keep a monotonic sequence so external listeners can detect stale updates.
self.sequence = self.sequence.wrapping_add(1);
```

Bad:
```rust
// Increment sequence by one.
self.sequence = self.sequence.wrapping_add(1);
```
