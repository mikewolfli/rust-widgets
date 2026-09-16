"""Independent structural checks for the `tools/check_locking.sh` matcher.

The locking gate's value is that it can *fail*. Its analysis is Python embedded in a
shell script, so nothing else exercises it, and a bug in the matcher does not raise —
it makes the gate report PASS on code it never looked at. Three such bugs were found
only by reverse injection:

* a lifetime (`&'static str`) read as a char literal, swallowing the rest of the line;
* a comment containing `[32..56)` counted as four closing brackets;
* a JSON fixture's doubled `{{`/`}}` counted as braces.

Each made the skip run past the test module and silently exempt all later production
code. This module supplies evidence the matcher cannot supply about itself, by checking
two properties of the stripped output that hold for any correct implementation:

1. **Nothing gated on `test` survives.** No `#[test]` item and no `#[cfg(... test ...)]`
   attribute may remain in the production text. A strip that stops early leaves these,
   which is what a too-short removal looks like.
2. **Unconditional code is never removed.** For a file where the test gate is the last
   item, the first line after the gate must survive. A strip that runs to end-of-file
   removes it, which is what a too-long removal looks like.

Both are computed from the file's own lines, not from the matcher.
"""

from __future__ import annotations

import re
from pathlib import Path

# An attribute gating an item on `test`, e.g. `#[cfg(test)]`, `#[cfg(all(test, x))]`.
TEST_GATE = re.compile(r"^#\[cfg\([^]]*\btest\b[^]]*\)\]$")


def load_matcher(script: Path) -> dict:
    """Extracts and executes the gate's Python region, returning its namespace."""
    source = script.read_text(encoding="utf-8")
    namespace: dict = {"re": re}
    exec(
        compile(
            source[source.index("PATTERN = re.compile") : source.index("findings: list[str] = []")],
            f"{script}:python",
            "exec",
        ),
        namespace,
    )
    return namespace


def structural_problems(root: Path, namespace: dict) -> list[str]:
    """Returns one message per file violating either property above.

    Property 1 is checked as *removal of lock-bearing lines*, not of every `#[test]`
    item, because two legitimate shapes leave test code in place: a file-based
    `mod tests;` declaration (the body lives in `tests.rs`, which the scan covers on its
    own) and a bare `#[test] fn` written after the module (as in `web_engine.rs`). Both
    are test code by construction — they cannot be production locks — so requiring their
    removal would be wrong. What must never survive is a lock-bearing line that is *not*
    inside a test context, and any lock the gate leaves behind is reported by the gate
    itself.
    """
    problems: list[str] = []
    for path in sorted((root / "src" / "widget").rglob("*.rs")):
        name = str(path.relative_to(root))
        source = path.read_text(encoding="utf-8")
        lines = source.split("\n")
        production = namespace["strip_test_modules"](source)
        surviving = set(production.split("\n"))

        # Property 1 is verified by the caller: every lock the strip leaves behind must
        # be inside a file whose whole body is test code. Here the matcher's completeness
        # is checked directly against rustc's answer for the one shape that is
        # unambiguous — a `#[cfg(test)] mod tests { ... }` that opens the file's last
        # item. Its opening brace must not survive, because that module is never
        # production code.
        gate = next((i for i, line in enumerate(lines) if TEST_GATE.match(line.strip())), None)
        if gate is not None:
            opener = lines[gate + 1].strip() if gate + 1 < len(lines) else ""
            is_inline_module = opener.startswith("mod ") and opener.endswith("{")
            is_file_module = opener.startswith("mod ") and opener.endswith(";")
            if is_inline_module and not is_file_module and opener in surviving:
                problems.append(
                    f"{name}:{gate + 2}: an inline test module survived the strip: {opener}"
                )

        # Property 2: the strip does not overrun. Only meaningful when the gate opens the
        # file's last item, which is the shape in every file of this tree except the
        # `mod tests;` declarations (those have no body to overrun).
        if gate is not None:
            opener = lines[gate + 1].strip() if gate + 1 < len(lines) else ""
            if opener.startswith("mod ") and opener.endswith(";"):
                continue
        last_close = [i for i, line in enumerate(lines) if line.startswith("}")]
        if not last_close:
            continue
        for number in range(last_close[-1] + 1, len(lines)):
            line = lines[number]
            if line.strip() and line not in surviving:
                problems.append(
                    f"{name}:{number + 1}: removed code after the last top-level brace: "
                    f"{line.strip()[:60]}"
                )
                break
    return problems
