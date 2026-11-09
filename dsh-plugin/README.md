# dsh-skill-rust-widgets

A DeepSeek Harness (`dsh`) skill plugin for maintaining, extending and auditing
the [rust-widgets](https://github.com/mikewolfli/rust-widgets) Rust GUI library.

Installing the bundle is the whole setup. It registers one skill provider, so
`rust-widgets-maintainer` appears in the session catalog with no profile
configuration, no environment variables and no prompt changes.

## Install

One command, no configuration:

```sh
dsh plugin --profile <profile> add "git+https://github.com/mikewolfli/rust-widgets.git#main&path:/dsh-plugin"
```

`<profile>` is the profile you boot (`web`, `tui`, ...). The bundle patch enables
the skill, so `rust-widgets-maintainer` appears in the catalog as soon as the
profile starts. Replace `main` with a commit id to pin a revision.

For a local checkout before committing:

```sh
dsh plugin --profile <profile> add /absolute/path/to/rust-widgets/dsh-plugin
```

Then start the profile as usual (`dsh <profile>`).

## What the skill does

`rust-widgets-maintainer` routes work on the rust-widgets repository:

- the governing rules in `docs/plans/principle.md`,
- the repository map (controls, layout, render, platform, code editor, gates),
- the standard edit → focused check → full validation workflow,
- control/capability registration steps,
- the code-editor language seam (including Zig),
- the reporting format every task must end with.

## Requirements

- DSH `>= 0.2.2-rc.1`
- Node `>= 22`

The package has no runtime dependencies.

## Development

```sh
npm run sync-skill   # copy .dsh/skills into ./assets and rebuild index.json
npm run check-skill  # fail if the bundled copy drifted
npm run check-index  # fail if market-index.json drifted from the package
npm test             # node:test against the provider contract
npm pack --dry-run   # inspect the published file list
```

The source of truth is `<repo>/.dsh/skills/rust-widgets-maintainer/SKILL.md`.
`prepack` re-syncs it automatically before `npm publish`, so the published copy
cannot lag behind the repository.

## Market index

The DSH plugin market has no official index: **Discover lists only the index
URLs a user adds.** This repository hosts a format-1 index for the plugin:

```
https://cdn.jsdelivr.net/gh/mikewolfli/rust-widgets@main/dsh-plugin/market-index.json
```

Add it once under **Plugins → Discover**, then install from the card. The card's
entry pins an exact commit, so that install is reproducible.

Regenerate the index whenever the package changes:

```sh
npm run market-index -- --git   # git source, pinned to the package's last commit
npm run market-index            # npm source, after `npm publish`
npm run check-index             # drift gate for CI
```

`--git` pins the newest commit that changed `dsh-plugin/` (the index file itself
does not move the pin, so regenerating is idempotent) and refuses an uncommitted
package, so commit the package first.

## How it works

`lib/index.js` is a Cordis plugin: it registers a provider on `ctx.skills` with
`list()` and `get()`. The bundled `assets/index.json` carries each skill's name,
description and `whenToUse`; the body is read from `assets/<name>/SKILL.md` on
demand and the YAML frontmatter is stripped because the catalog already holds
those fields.

The bodies sit under `assets/` rather than `skills/` on purpose: package managers
that treat a dependency's `skills/` directory as agent content would otherwise
gate the install behind an approval prompt. The canonical copy still lives at
`<repo>/.dsh/skills/rust-widgets-maintainer/SKILL.md`.
