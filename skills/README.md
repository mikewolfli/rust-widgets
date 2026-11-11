# dsh skill interface for `rust-widgets`

This directory documents the project's DeepSeek Harness (`dsh`) skill. The
**loadable** skill lives at:

```
.dsh/skills/rust-widgets-maintainer/SKILL.md
```

`dsh` does not scan a top-level `skills/` directory. Its local filesystem
provider uses these project roots (nearest ancestor containing `.git`):

| Rank | Source | Root |
|---|---|---|
| 100 | `project-dsh` | `<projectRoot>/.dsh/skills` |
| 200 | `project-agents` | `<projectRoot>/.agents/skills` |

Within a root it accepts either a directory bundle `<name>/SKILL.md` or a flat
`<name>.md`. Nested recursive `**/SKILL.md` discovery is **not** supported, so the
skill sits directly at `.dsh/skills/rust-widgets-maintainer/SKILL.md` (rank 100).

## Frontmatter contract (exact keys)

`dsh` parses YAML frontmatter and requires:

- `name` — kebab-case, matching `^[a-z0-9]+(?:-[a-z0-9]+)*$`
- `description` — non-empty routing text

Optional keys:

- `whenToUse` — extra routing guidance (camelCase, exactly this spelling)
- `disable-model-invocation` — boolean, default `false`
- `user-invocable` — boolean, default `true`
- `metadata` — arbitrary object, preserved verbatim

Legacy spellings (`disableModelInvocation`, `modelInvocable`, `userInvocable`)
are rejected. The skill uses the defaults, so both the model and the user can
invoke it.

## `manifest.json`

`dsh` does **not** read `manifest.json`. The file at
`skills/rust-widgets/manifest.json` is a machine-readable **mirror** of the
discovery contract above (roots, ranks, frontmatter keys, skill path), kept for
tooling and for runtimes that need a JSON description. The `SKILL.md` remains the
single source of truth.

## Verify locally

From the repository root, start `dsh`; its first `agent/pre-step` should list
`rust-widgets-maintainer` in the skill catalog. The skill body is loaded on
demand through the model-facing `skill` tool, not injected into every prompt.

The skill can also be read directly by a human or any agent:
[`.dsh/skills/rust-widgets-maintainer/SKILL.md`](../.dsh/skills/rust-widgets-maintainer/SKILL.md).
