// Generate a format-1 DSH plugin market index.
//
// DSH has no official index: Discover only reads the index URLs a user adds.
// This file is the index you host.
//
// Two source modes, because an index entry is either an npm package or a Git
// commit:
//
//   npm run market-index           npm source; requires the version to be published
//   npm run market-index -- --git  git source pinned to the commit that carries
//                                  this package (works without npm publishing)
//
// In git mode the recorded `publishTimestamp` is the commit time, the closest
// honest equivalent of an npm publish time. The commit must already contain the
// package files, so commit first and generate after.

import { execFileSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { basename, dirname, relative, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

const PACKAGE_PATH = fileURLToPath(new URL('../package.json', import.meta.url))
const OUTPUT_PATH = fileURLToPath(new URL('../market-index.json', import.meta.url))
const PACKAGE_DIR = dirname(PACKAGE_PATH)
const REPO_ROOT = dirname(PACKAGE_DIR)
/** The package's POSIX path inside its repository; absent means the repository root. */
const PACKAGE_DIR_IN_REPO = relative(REPO_ROOT, PACKAGE_DIR).split(sep).join('/')
const GIT_MODE = process.argv.includes('--git')
const CHECK = process.argv.includes('--check')
const COMMIT_PATTERN = /^[0-9a-f]{40}$/

const packageJson = JSON.parse(readFileSync(PACKAGE_PATH, 'utf8'))
const { name, version, description } = packageJson

function fail(message) {
  console.error(`dsh-skill-rust-widgets: ${message}`)
  process.exit(1)
}

function run(command, args, cwd) {
  const binary = process.platform === 'win32' && command === 'npm' ? 'npm.cmd' : command
  return execFileSync(binary, args, { cwd, encoding: 'utf8' })
}

function git(args) {
  return run('git', args, REPO_ROOT).trim()
}

/** The repository's web page without the `git+` prefix or the `.git` suffix. */
const repositoryUrl = packageJson.repository.url.replace(/^git\+/, '').replace(/\.git$/, '')

/** The entry's source plus the time it became installable. */
function npmSource() {
  let times
  try {
    times = JSON.parse(run('npm', ['view', name, 'time', '--json'], PACKAGE_DIR))
  } catch {
    fail(`cannot read npm publish times for ${name}; publish the package first, or use --git`)
  }
  const published = times?.[version]
  if (typeof published !== 'string') {
    fail(`${name}@${version} is not published yet (npm has no publish time for it); use --git`)
  }
  const publishTimestamp = Math.floor(Date.parse(published) / 1000)
  if (!Number.isFinite(publishTimestamp) || publishTimestamp <= 0) {
    fail(`npm returned an unreadable publish time for ${name}@${version}: ${published}`)
  }
  return { publishTimestamp, source: { type: 'npm' } }
}

/** The package directory's files, minus the index this script owns. */
const PACKAGE_PATHSPEC = [PACKAGE_DIR_IN_REPO, `:(exclude)${PACKAGE_DIR_IN_REPO}/${basename(OUTPUT_PATH)}`]

/** The source and time of the last commit that changed the package. */
function gitSource() {
  const dirty = git(['status', '--porcelain', '--', ...PACKAGE_PATHSPEC])
  if (dirty !== '') {
    fail(`uncommitted changes under ${PACKAGE_DIR_IN_REPO}; commit the package first:\n${dirty}`)
  }

  // Pin the newest commit that changed the package, not HEAD: the index itself
  // is not a package input, so rewriting it must not move the pin.
  let commit
  try {
    commit = git(['log', '-1', '--format=%H', '--', ...PACKAGE_PATHSPEC])
  } catch {
    fail(`cannot read the commit history of ${PACKAGE_DIR_IN_REPO}`)
  }
  if (!COMMIT_PATTERN.test(commit)) fail(`no commit carries ${PACKAGE_DIR_IN_REPO}`)

  // Refuse to point the index at a commit whose package is a different version:
  // the pin would install something the index does not describe.
  let committed
  try {
    committed = JSON.parse(git(['show', `${commit}:${PACKAGE_DIR_IN_REPO}/package.json`]))
  } catch {
    fail(`${commit} does not carry ${PACKAGE_DIR_IN_REPO}/package.json; commit the package first`)
  }
  if (committed.name !== name || committed.version !== version) {
    fail(
      `${commit} carries ${committed.name}@${committed.version}, not ${name}@${version}; commit the package first`,
    )
  }

  const publishTimestamp = Number(git(['show', '-s', '--format=%ct', commit]))
  if (!Number.isInteger(publishTimestamp) || publishTimestamp <= 0) {
    fail(`cannot read the commit time of ${commit}`)
  }
  return {
    publishTimestamp,
    source: { type: 'git', url: repositoryUrl, commit, path: PACKAGE_DIR_IN_REPO },
  }
}

const { publishTimestamp, source } = GIT_MODE ? gitSource() : npmSource()

const index = {
  format: 1,
  title: 'rust-widgets DSH skills',
  description: 'Skills for maintaining the rust-widgets Rust GUI library.',
  metadata: {
    categories: { development: { title: 'Development' } },
    labels: {},
    sortKeys: {},
  },
  plugins: [
    {
      name,
      title: 'rust-widgets maintainer',
      description,
      // Resolved against this index's URL, which is hosted beside the icon.
      icon: `./${basename(packageJson.icon ?? 'icon.svg')}`,
      author: packageJson.author,
      homepage: packageJson.homepage,
      repository: { url: repositoryUrl, directory: packageJson.repository.directory },
      metadata: {
        category: 'development',
        keywords: packageJson.keywords ?? [],
        labels: [],
        sortKeys: {},
      },
      versions: [
        {
          version,
          publishTimestamp,
          source,
          engines: { dsh: packageJson.engines?.dsh },
        },
      ],
    },
  ],
}

const text = `${JSON.stringify(index, null, 2)}\n`

if (CHECK) {
  let current
  try {
    current = readFileSync(OUTPUT_PATH, 'utf8')
  } catch {
    fail(`${OUTPUT_PATH} is missing; run: npm run market-index${GIT_MODE ? ' -- --git' : ''}`)
  }
  if (current !== text) {
    fail(`${OUTPUT_PATH} is out of date; run: npm run market-index${GIT_MODE ? ' -- --git' : ''}`)
  }
  console.log(`index ok: ${name}@${version} (${source.type} source)`)
  process.exit(0)
}

writeFileSync(OUTPUT_PATH, text)
console.log(`wrote ${OUTPUT_PATH} (${source.type} source)`)
