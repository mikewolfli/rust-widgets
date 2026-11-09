// Copy the repository's canonical skills into this package before packing.
//
// Source of truth: <repo>/.dsh/skills/<name>/SKILL.md (the root DSH discovery
// tree). Destination: ./assets/<name>/SKILL.md plus a small index.json the
// runtime reads without a YAML parser. `--check` verifies the two trees match
// without writing, so CI or a maintainer can catch drift.
//
// The bundled bodies live under `assets/` on purpose: package managers that
// sniff a dependency's `skills/` directory as agent content would otherwise
// gate the install behind an approval prompt, and this package needs none.

import {
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const SOURCE_ROOT = fileURLToPath(new URL('../../.dsh/skills/', import.meta.url))
const DEST_ROOT = fileURLToPath(new URL('../assets/', import.meta.url))
const NAME_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/
const CHECK = process.argv.includes('--check')

function fail(message) {
  console.error(`dsh-skill-rust-widgets: ${message}`)
  process.exit(1)
}

function unquote(value) {
  if (
    (value.startsWith('"') && value.endsWith('"')) ||
    (value.startsWith("'") && value.endsWith("'"))
  ) {
    return value.slice(1, -1)
  }
  return value
}

/** Read `name` and `description` from the SKILL.md frontmatter. */
function parseFrontmatter(raw, path) {
  const match = /^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/u.exec(raw)
  if (!match) fail(`${path} has no YAML frontmatter`)
  const fields = {}
  let key = null
  for (const line of match[1].split(/\r?\n/)) {
    if (!line.trim()) continue
    if (/^\s+/.test(line)) {
      if (key !== null) fields[key] = `${fields[key]} ${line.trim()}`.trim()
      continue
    }
    const field = /^([A-Za-z0-9_-]+):\s*(.*)$/.exec(line)
    if (!field) continue
    key = field[1]
    const value = field[2].trim()
    fields[key] = value === '>' || value === '|' ? '' : unquote(value)
  }
  return fields
}

const skills = []
for (const entry of readdirSync(SOURCE_ROOT, { withFileTypes: true })) {
  if (!entry.isDirectory()) continue
  const path = join(SOURCE_ROOT, entry.name, 'SKILL.md')
  let raw
  try {
    raw = readFileSync(path, 'utf8')
  } catch {
    continue
  }
  const frontmatter = parseFrontmatter(raw, path)
  const skillName = frontmatter.name
  const description = frontmatter.description
  if (typeof skillName !== 'string' || !NAME_PATTERN.test(skillName)) {
    fail(`${path} has an invalid name: ${JSON.stringify(skillName)}`)
  }
  if (skillName !== entry.name) {
    fail(`${path}: frontmatter name ${skillName} does not match its directory ${entry.name}`)
  }
  if (typeof description !== 'string' || description.length === 0) {
    fail(`${path} has no description`)
  }
  const whenToUse = frontmatter.whenToUse
  if (whenToUse !== undefined && whenToUse.trim().length === 0) {
    fail(`${path} has an empty whenToUse`)
  }
  skills.push({
    name: skillName,
    description,
    directory: skillName,
    whenToUse: whenToUse === undefined ? undefined : whenToUse.trim(),
    raw,
  })
}

if (skills.length === 0) fail(`no skills found under ${SOURCE_ROOT}`)
skills.sort((left, right) => left.name.localeCompare(right.name))

const index = {
  skills: skills.map(({ name, description, directory, whenToUse }) => ({
    name,
    description,
    directory,
    ...(whenToUse === undefined ? {} : { whenToUse }),
  })),
}
const indexText = `${JSON.stringify(index, null, 2)}\n`

if (CHECK) {
  const problems = []
  for (const skill of skills) {
    const dest = join(DEST_ROOT, skill.directory, 'SKILL.md')
    let current
    try {
      current = readFileSync(dest, 'utf8')
    } catch {
      problems.push(`${dest} is missing`)
      continue
    }
    if (current !== skill.raw) problems.push(`${dest} is out of date`)
  }
  let currentIndex
  try {
    currentIndex = readFileSync(join(DEST_ROOT, 'index.json'), 'utf8')
  } catch {
    problems.push(`${join(DEST_ROOT, 'index.json')} is missing`)
  }
  if (currentIndex !== undefined && currentIndex !== indexText) {
    problems.push(`${join(DEST_ROOT, 'index.json')} is out of date`)
  }
  if (problems.length > 0) {
    fail(`bundled skills are stale:\n  ${problems.join('\n  ')}\nrun: npm run sync-skill`)
  }
  console.log(`check ok: ${skills.length} skill(s) in sync`)
  process.exit(0)
}

mkdirSync(DEST_ROOT, { recursive: true })
for (const skill of skills) {
  const directory = join(DEST_ROOT, skill.directory)
  mkdirSync(directory, { recursive: true })
  writeFileSync(join(directory, 'SKILL.md'), skill.raw)
}
writeFileSync(join(DEST_ROOT, 'index.json'), indexText)
for (const entry of readdirSync(DEST_ROOT, { withFileTypes: true })) {
  if (entry.isDirectory() && !skills.some((skill) => skill.directory === entry.name)) {
    rmSync(join(DEST_ROOT, entry.name), { recursive: true, force: true })
  }
}
console.log(`synced ${skills.length} skill(s) into ${DEST_ROOT}`)
