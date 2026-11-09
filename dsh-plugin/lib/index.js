// The DSH skill provider for the rust-widgets maintainer skill.
//
// The package ships the skill under `skills/<name>/SKILL.md` and a prebuilt
// `skills/index.json`. The provider reads that index synchronously when the
// profile mounts the plugin, then loads the instruction body on demand. There
// is no configuration and no build step at install time: installing the bundle
// is the whole setup.

import { readFileSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import { join, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const SKILL_ROOT = fileURLToPath(new URL('../skills/', import.meta.url))
const INDEX_PATH = join(SKILL_ROOT, 'index.json')

/** Provider identity, recorded on every candidate the registry holds. */
const PROVIDER = 'dsh-rust-widgets'
/** Bundled-provider rank; the filesystem provider uses the same rank for bundled roots. */
const RANK = 600

/** Cordis plugin identity. */
export const name = 'rust-widgets-skill'
/** The registry this plugin registers a provider on. */
export const inject = ['skills']

function readCatalog() {
  let index
  try {
    index = JSON.parse(readFileSync(INDEX_PATH, 'utf8'))
  } catch (error) {
    throw new Error(`dsh-skill-rust-widgets: cannot read ${INDEX_PATH}: ${error.message}`)
  }
  if (!Array.isArray(index.skills) || index.skills.length === 0) {
    throw new Error(`dsh-skill-rust-widgets: ${INDEX_PATH} lists no skills`)
  }
  return index.skills.map((skill) => {
    const directory = join(SKILL_ROOT, skill.directory)
    return {
      name: skill.name,
      description: skill.description,
      ...(skill.whenToUse === undefined ? {} : { whenToUse: skill.whenToUse }),
      invocation: { modelInvocable: true, userInvocable: true },
      provider: PROVIDER,
      source: 'bundled',
      rank: RANK,
      resourceBase: { kind: 'directory', path: directory.split(sep).join('/') },
      locator: join(directory, 'SKILL.md'),
    }
  })
}

/** Remove the leading YAML frontmatter block; the catalog already carries its fields. */
function stripFrontmatter(raw) {
  const match = /^---\r?\n[\s\S]*?\r?\n---(?:\r?\n|$)/u.exec(raw)
  return (match ? raw.slice(match[0].length) : raw).trim()
}

/**
 * Register the bundled skill provider.
 * @param ctx - context carrying the skill registry (`ctx.skills`).
 */
export function apply(ctx) {
  const candidates = readCatalog()
  const provider = {
    name: PROVIDER,
    list: () => Promise.resolve(candidates),
    async get(candidate, options) {
      const { rank: _rank, locator, ...summary } = candidate
      let raw
      try {
        raw = await readFile(locator, { encoding: 'utf8', signal: options?.signal })
      } catch (error) {
        // The registry contract asks for `undefined` once a listed body is no
        // longer loadable; an aborted lookup is not a missing skill, so rethrow.
        if (error?.name === 'AbortError') throw error
        return undefined
      }
      return { ...summary, content: stripFrontmatter(raw) }
    },
  }
  ctx.skills.registerProvider(() => provider)
}

export default apply
