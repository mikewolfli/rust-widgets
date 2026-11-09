import assert from 'node:assert/strict'
import test from 'node:test'

import { apply, inject, name } from '../lib/index.js'

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
