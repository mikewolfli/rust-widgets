// The plugin ships with the library, so its version is not its own: it is the
// root package's `Cargo.toml` version. Two copies of a version number drift; one
// copy cannot. Every script that writes or checks a release artifact reads this
// module instead of trusting `package.json` on its own.

import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

/** The root library manifest, two directories above the plugin package. */
export const LIBRARY_MANIFEST = fileURLToPath(new URL('../../Cargo.toml', import.meta.url))

/**
 * Read the `[package] version` from a Cargo manifest.
 * @param {string} manifestPath - the manifest to read.
 * @returns {string} the version string.
 */
export function libraryVersion(manifestPath = LIBRARY_MANIFEST) {
  const lines = readFileSync(manifestPath, 'utf8').split(/\r?\n/u)
  let inPackage = false
  for (const line of lines) {
    const trimmed = line.trim()
    if (/^\[[^\]]+\]$/u.test(trimmed)) {
      inPackage = trimmed === '[package]'
      continue
    }
    if (!inPackage) continue
    const version = /^version\s*=\s*"([^"]+)"$/u.exec(trimmed)
    if (version) return version[1]
  }
  throw new Error(`${manifestPath} declares no [package] version`)
}

/**
 * Assert that a package manifest tracks the library version.
 * @param {{version?: string}} packageJson - the parsed plugin package manifest.
 * @param {string} manifestPath - the library manifest to compare against.
 * @returns {string} the shared version.
 */
export function assertPackageMatchesLibrary(packageJson, manifestPath = LIBRARY_MANIFEST) {
  const expected = libraryVersion(manifestPath)
  if (packageJson.version !== expected) {
    throw new Error(
      `dsh-skill-rust-widgets: package version ${JSON.stringify(packageJson.version)} does not ` +
        `match the library's ${expected}; set package.json#version to ${expected}`,
    )
  }
  return expected
}
