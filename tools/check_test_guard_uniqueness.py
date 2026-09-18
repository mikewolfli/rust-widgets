#!/usr/bin/env python3
"""A test guard must delegate to the shared lock, not declare its own.

# The defect this blocks

`embedded_target_fps_clamps` failed intermittently. The cause was a **second**
`static OnceLock<Mutex<()>>`: `render_engine::embedded` owned the canonical
`embedded_test_guard`, while `render_engine::embedded_engine` declared its own
`fn test_guard()` holding a *different* mutex over the same process-wide embedded
engine. Two locks over one resource exclude nothing, so `set_embedded_target_fps(120)`
from one module landed inside the other module's assertion on 72.

It read as a flake because the overlap window is nanoseconds: the writer must schedule
between `set_embedded_target_fps(72)` and the following `embedded_target_fps()` read.
It passed on retry every time. Widening that span by 400 ms reproduced it immediately
(`left: 120, right: 72`), which is how the cause was identified rather than guessed.

# The rule

A "test guard" is a function returning `MutexGuard<'static, ()>`. Exactly **one** such
function may *declare* the `static ... OnceLock<Mutex<()>>` it locks — the canonical,
`pub` / `pub(crate)` `<something>_test_guard`. Every other guard must **delegate** to
it (a local wrapper is fine and keeps call sites readable; a local `static` is not).

`OnceLock<Mutex<T>>` holding *state* (a registry, a config) is a singleton, not a
guard, and is not reported — there is exactly one of each by construction.
"""

import pathlib
import re
import sys

SRC = pathlib.Path('src')

# A guard's signature: it returns a guard over the unit type.
GUARD_SIG = re.compile(r'->\s*[A-Za-z_:]*MutexGuard<\s*(?:\'static\s*,)?\s*\(\s*\)\s*>')
# A declaration of its own lock.
DECLARES_LOCK = re.compile(r'static\s+\w+\s*:\s*OnceLock<\s*Mutex<\s*\(\s*\)\s*>')


def guard_functions() -> list[tuple[str, int, str, bool, bool]]:
    """(file, line, name, is_public, declares_own_lock) for every test-guard fn."""
    found = []
    for path in sorted(SRC.rglob('*.rs')):
        text = path.read_text(encoding='utf-8')
        for match in re.finditer(r'\bfn\s+(?P<name>\w+)\s*(?:<[^>]*>)?\s*\(\s*\)', text):
            # The body ends at the next `\n}`; slice a bounded window for the checks.
            window = text[match.start() : match.start() + 900].split('\n}\n')[0]
            if not GUARD_SIG.search(window):
                continue
            line = text[: match.start()].count('\n') + 1
            prefix = text[max(0, match.start() - 60) : match.start()]
            is_public = bool(re.search(r'pub(?:\(crate\))?\s*$', prefix))
            found.append(
                (
                    str(path),
                    line,
                    match.group('name'),
                    is_public,
                    bool(DECLARES_LOCK.search(window)),
                )
            )
    return found


def main() -> int:
    guards = guard_functions()

    # Reverse assertion (principle #77): an empty scan is a broken scan, not a clean tree.
    if not guards:
        print(
            'check_test_guard_uniqueness: FAILED — found no test-guard functions at all, so '
            'this gate is not reading the sources it claims to',
            file=sys.stderr,
        )
        return 2

    # A local guard is one that declares its own lock without being the shared entry
    # point. Naming is the signal for "shared": the canonical guard is `pub` and named
    # `<something>_test_guard`.
    local = [
        (path, line, name)
        for path, line, name, is_public, declares in guards
        if declares and not (is_public and name.endswith('_test_guard'))
    ]

    if local:
        print(
            'These functions declare their own `OnceLock<Mutex<()>>` instead of delegating '
            'to the canonical `pub *_test_guard`, so they are a second lock over shared '
            'state — two locks over one resource exclude nothing:',
            file=sys.stderr,
        )
        for path, line, name in local:
            print(f'  ❌ {path}:{line}: `fn {name}()`', file=sys.stderr)
        print(
            '\nFix: delete the local static and delegate to the canonical guard, so every '
            'module driving the same singleton takes the same lock.',
            file=sys.stderr,
        )
        return 1

    delegating = sorted(name for _, _, name, _, declares in guards if not declares)
    canonical = sorted(
        name for _, _, name, is_public, declares in guards if declares and is_public
    )
    detail = f'{len(canonical)} canonical ({", ".join(canonical)})'
    if delegating:
        detail += f', {len(delegating)} delegating ({", ".join(delegating)})'
    print(f'✅ check_test_guard_uniqueness: {detail} — no module-local locks over shared state')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
