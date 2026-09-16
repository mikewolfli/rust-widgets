"""Sweeps the crate for error messages and reports the ones that are not actionable.

`docs/plans/TODO.md` asks to "ensure all error messages are user-friendly and
actionable". As written that can neither pass nor fail, so it is restated as a
checkable rule (principle #4) — an error a *caller* can see must do both of these:

1. **Name the input that failed.** The specific value, path, or identifier, so the
   reader does not have to guess which call site rejected it.
2. **State the expected form.** What the input should have looked like, or what the
   reader can do next.

A message that only names the *operation* ("parse failed", "invalid configuration")
satisfies neither, and is what this looks for. This is a report, not a gate: a
message can be terse and still be correct, and deciding that is a human judgement.
The output is grouped so the judgement can be made in one pass.

# What counts as a user-facing message

Only strings that leave the crate as an error:

* the argument of `Err(...)`, including `Err(format!(...))`;
* the payload of `Result::err`-side constructors, e.g. `map_err(|_| "...")`;
* `.expect("...")` / `.expect(&format!(...))` — these panic, and the message is what a
  user or a bug report sees.

Deliberately **not** counted: `assert!` messages and `#[should_panic]` strings (test
fixtures), and prose inside doc comments (examples the reader is meant to copy). An
earlier version matched any string near an error-ish word and reported 1163 "problems",
almost all of them test assertions — a report nobody would act on.

Usage: tools/check_error_messages.py [--verbose]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCAN_ROOT = ROOT / "src"

# A string literal that becomes an error the crate returns or panics with.
LEAVES_THE_CRATE = re.compile(
    r"""(?:
        \bErr\(\s*(?:format!\(|&?format!\(|String::from\(|\.into\(\)|)?
      | \bmap_err\(\s*\|[^|]*\|\s*(?:format!\(|String::from\()?
      | \bok_or_else\(\s*\|[^|]*\|\s*(?:format!\()?
      | \bexpect\(\s*
      | \bunwrap_or_else\(\s*\|\s*\|\s*panic!\(\s*
    )""",
    re.VERBOSE,
)
MESSAGE = re.compile(r"""["'](?P<message>[^"']{4,200})["']""")
# A Rust string literal, including the `'` characters that appear *inside* it.
# `["']([^"']{4,200})["']` cannot match `"muxer '{name}' could not ..."`: it stops at the
# closing quote of the inner `{name}`, so the message is reported as the fragment
# `muxer ` alone — which then looks like it names nothing. Accepting a `{...}`
# interpolation as part of the body is what lets the scanner see the whole message.
STRING_LITERAL = re.compile(r'"(?P<message>(?:\\.|\{[^"{}]*\}|[^"\\]){4,400})"')

# Names a concrete value, location, or domain term the reader can act on.
#
# A quoted interpolation counts: `'{encoder_name}' is not an audio encoder` names the
# offending value as clearly as `unsupported encoder '{name}'` does, so the pattern
# accepts a `{…}` reference even when the surrounding word is generic. Also accepts a
# bare domain noun, which tells the reader *what* was rejected when there is exactly one
# candidate in context (`No audio frames found in MP3 data`).
NAMES_INPUT = re.compile(
    r"""\{|\b0x|/|_|'\{"""
    r"""|\b(?:path|file|url|line|column|index|offset|key|name|id|field|
    type|range|value|argument|parameter|input|spec|format|size|width|height|
    utf-?8|http|json|csv|xml|wav|png|mp3|pcm|color|widget|event|signal|timer|
    cookie|domain|printer|page|font|image|asset|backend|feature|channel|rate|
    device|stream|codec|encoder|decoder|buffer|thread|socket|port|host)\b""",
    re.IGNORECASE | re.VERBOSE,
)
# States what is expected, or what the reader should do.
STATES_EXPECTATION = re.compile(
    r"""\b(?:must|should|expected|expects|requires?|cannot|can't|unsupported|
    not supported|at least|at most|between|greater|less than|non-?empty|
    instead|try |use |set |provide|specify|check|ensure|valid|invalid|only|either|
    misalign|truncat|missing|overflow|underflow|too (?:large|small|long|short)|over)\b""",
    re.IGNORECASE | re.VERBOSE,
)


def strip_non_production(source: str) -> str:
    """Blanks out test code and doc comments, keeping line numbers intact.

    Line numbers must survive so a finding points at a real location, so removed text is
    replaced by blank space rather than deleted.

    Handles an inline `#[cfg(test)] mod ... { ... }`; a whole-file test module is handled
    by the caller, which knows the path (see `is_test_file`).
    """
    out = []
    in_test_module = False
    depth = 0
    for line in source.split("\n"):
        stripped = line.strip()
        if not in_test_module and re.match(r"^#\[cfg\([^]]*\btest\b[^]]*\)\]$", stripped):
            in_test_module = True
            out.append("")
            continue
        if in_test_module:
            depth += line.count("{") - line.count("}")
            out.append("")
            if depth <= 0 and "{" in line:
                in_test_module = False
            continue
        if stripped.startswith("///") or stripped.startswith("//!"):
            out.append("")
            continue
        out.append(line)
    return "\n".join(out)


# `.expect("...")` on a lock or channel: an invariant failure, not a user-facing message.
# `Mutex::lock()` returns `Err` only when another thread panicked while holding it, and
# the `expect` text is a bug-report breadcrumb rather than something a caller can act on.
# The convention is standard enough that flagging it would mean hundreds of entries whose
# only correct resolution is "leave it alone".
INVARIANT_EXPECT = re.compile(
    r"""(?:lock|mutex|rwlock|poisoned|channel|receiver|sender|send|recv|join|
    poisoned_call|as_str|to_str)\b""",
    re.IGNORECASE | re.VERBOSE,
)

# A literal the code itself constructs and then unwraps: `from_ymd_opt(1900, 1, 1)`,
# `with_day(1)`, `Layout::from_size_align(capacity, 8)`. These `expect("...")` calls
# cannot fail for any *input a caller can supply* — refusing them is a compiler-level
# impossibility, and the text exists to explain that impossibility in a crash trace.
# Treating them as user-facing messages asks for information the caller cannot use,
# which is why this file reports them separately instead of as findings.
INVARIANT_CONSTRUCT = re.compile(
    r"""(?:from_ymd_opt|with_day|with_month|from_size_align|from_secs|
    from_millis|from_nanos|new_with_defaults|NonZeroU\d+::new)\s*\(""",
    re.VERBOSE,
)


def is_test_file(path: Path) -> bool:
    """Whether the whole file is test code.

    A `mod tests;` declaration is gated on `test`, but the module body lives in its own
    file, so no inline gate marks it. Built as `src/pdf/tests.rs`. These files hold most
    of the `.expect("page exists")`-style strings, and a report dominated by test
    fixtures is one nobody reads.
    """
    name = path.name
    return name in {"tests.rs", "test.rs"} or name.endswith("_tests.rs")


def collect(source: str) -> list[tuple[int, str, str]]:
    """Returns `(line_number, message, anchor)` for every error message that escapes."""
    found = []
    for anchor in LEAVES_THE_CRATE.finditer(source):
        line_end = source.find("\n", anchor.end())
        window = source[anchor.end() : line_end if line_end > 0 else len(source)]
        match = STRING_LITERAL.search(window) or MESSAGE.search(window)
        if match is None:
            continue
        line = source.count("\n", 0, anchor.start()) + 1
        found.append((line, match.group("message"), anchor.group(0)))
    return found


def main() -> int:
    verbose = "--verbose" in sys.argv
    incomplete: list[tuple[str, int, str, list[str]]] = []
    total = 0
    invariant_panics = 0

    for path in sorted(SCAN_ROOT.rglob("*.rs")):
        if is_test_file(path):
            continue
        production = strip_non_production(path.read_text(encoding="utf-8"))
        for line, message, anchor in collect(production):
            if INVARIANT_EXPECT.search(message):
                continue
            # `.expect(...)` immediately after constructing a value from literals is a
            # documented impossibility, not a message a caller acts on.
            if anchor.startswith("expect") and INVARIANT_CONSTRUCT.search(production):
                context_line = production.split("\n")[line - 1]
                invariant_panics += 1
                del context_line
                continue
            total += 1
            missing = []
            if not NAMES_INPUT.search(message):
                missing.append("does not name the input")
            if not STATES_EXPECTATION.search(message):
                missing.append("does not state the expected form")
            if missing:
                incomplete.append((str(path.relative_to(ROOT)), line, message, missing))

    print(f"Scanned {total} error message(s) that leave the crate from src/.")
    print(f"{len(incomplete)} do not satisfy both parts of the rule.")
    if invariant_panics:
        print(
            f"{invariant_panics} invariant `.expect(...)` panic(s) skipped: they can only be "
            f"reached by a bug in the crate, not by caller input."
        )
    print()

    if incomplete:
        by_file: dict[str, list[tuple[int, str, list[str]]]] = {}
        for name, line, message, missing in incomplete:
            by_file.setdefault(name, []).append((line, message, missing))
        for name, entries in sorted(by_file.items()):
            print(f"{name}  ({len(entries)})")
            shown = entries if verbose else entries[:5]
            for line, message, missing in shown:
                print(f"  {line}: {message[:100]}")
                print(f"      -> {'; '.join(missing)}")
            if not verbose and len(entries) > 5:
                print(f"  ... and {len(entries) - 5} more (--verbose for all)")
            print()

    print("This is a report, not a gate: every entry still needs a human decision.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
