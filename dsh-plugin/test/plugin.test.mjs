import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { apply, inject, name } from '../lib/index.js'
import { libraryVersion } from '../scripts/version.mjs'

function mount() {
  let registered
  const ctx = {
    skills: {
      registerProvider(factory) {
        registered = factory()
      },
    },
  }
  apply(ctx)
  assert.ok(registered, 'apply() must register a provider')
  return registered
}

test('the plugin declares the skill registry it needs', () => {
  assert.equal(name, 'rust-widgets-skill')
  assert.deepEqual(inject, ['skills'])
})

test('the catalog exposes the bundled rust-widgets skill', async () => {
  const provider = mount()
  const candidates = await provider.list()
  assert.equal(candidates.length, 1)
  const [skill] = candidates
  assert.equal(skill.name, 'rust-widgets-maintainer')
  assert.ok(skill.description.length > 0)
  assert.match(skill.whenToUse, /rust-widgets repository/)
  assert.equal(skill.provider, 'dsh-rust-widgets')
  assert.equal(skill.source, 'bundled')
  assert.equal(skill.rank, 600)
  assert.equal(skill.invocation.modelInvocable, true)
  assert.equal(skill.invocation.userInvocable, true)
  assert.ok(skill.locator.endsWith('/SKILL.md'))
  assert.equal(skill.resourceBase.kind, 'directory')
})

test('loading returns the instruction body without frontmatter', async () => {
  const provider = mount()
  const [skill] = await provider.list()
  const loaded = await provider.get(skill, { signal: new AbortController().signal })
  assert.equal(loaded.name, 'rust-widgets-maintainer')
  assert.ok(!loaded.content.startsWith('---'), 'frontmatter must be stripped')
  assert.match(loaded.content, /^# rust-widgets maintainer/)
  assert.match(loaded.content, /docs\/plans\/principle\.md/)
})

test('a body that is no longer loadable resolves to undefined', async () => {
  const provider = mount()
  const [skill] = await provider.list()
  const missing = { ...skill, locator: skill.locator.replace(/SKILL\.md$/u, 'MISSING.md') }
  assert.equal(await provider.get(missing, {}), undefined)
})

test('the package version tracks the library version', () => {
  const packageJson = JSON.parse(
    readFileSync(new URL('../package.json', import.meta.url), 'utf8'),
  )
  assert.equal(packageJson.version, libraryVersion())
})

test('the skill covers every companion crate and the whole CLI surface', async () => {
  const provider = mount()
  const [skill] = await provider.list()
  const { content } = await provider.get(skill, {})
  // One entry per directory under `crates/`; a new crate without a line here is a
  // skill that cannot route work on it.
  for (const crate of [
    'rust-widgets-cli',
    'rust-widgets-suites',
    'rust-widgets-font',
    'rust-widgets-icons',
    'mobile-services',
  ]) {
    assert.match(content, new RegExp(crate, 'u'), `the skill must name ${crate}`)
  }
  for (const command of ['check', 'json2rust', 'fmt', 'create']) {
    assert.match(content, new RegExp(`\`rwui ${command}`, 'u'), `the skill must document rwui ${command}`)
  }
  assert.match(content, /default-features = false/u)
  assert.match(content, /package\.json#version/u)
})

test('the skill names the repository areas it is expected to maintain', async () => {
  const provider = mount()
  const [skill] = await provider.list()
  const { content } = await provider.get(skill, {})
  for (const area of ['crates/', 'demo/', 'examples/', 'dsh-plugin/', 'docs/plans/', 'tools/']) {
    assert.ok(content.includes(area), `the repository map must name ${area}`)
  }
})
