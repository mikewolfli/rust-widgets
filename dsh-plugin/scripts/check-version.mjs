// Fail when the plugin's version stops tracking the library's version.
//
// A run of this script is the whole check; it exists as a file rather than an
// inline `node -e` so `npm run check-version` and `npm test` share one message.

import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { assertPackageMatchesLibrary } from './version.mjs'

const packagePath = fileURLToPath(new URL('../package.json', import.meta.url))

try {
  const version = assertPackageMatchesLibrary(JSON.parse(readFileSync(packagePath, 'utf8')))
  console.log(`version ok: ${version} (matches the library)`)
} catch (error) {
  console.error(error.message)
  process.exit(1)
}
