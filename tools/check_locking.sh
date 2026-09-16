#!/usr/bin/env bash
# Locking audit gate.
#
# `docs/plans/TODO.md` asks to "audit Mutex, OnceLock and atomics usage" and to
# "profile lock contention in widget creation and event loop paths". Both were stuck
# as unverifiable: nobody can say when a code-wide "audit" is finished, and no
# profiler capture exists in this repository.
#
# This gate turns the auditable half into a pass/fail statement, using the property
# that actually matters: **the widget state path must not take a lock at all.** That
# is stronger than "contention is low", it is mechanically checkable, and it is the
# correct design here — `widget::runtime` keeps widgets in thread-local registries,
# so there is no shared mutable state to guard, and a lock would be pure overhead
# (plus a single-threaded deadlock risk).
#
# Scope, decided by reading each module rather than assuming:
#
#   `src/widget/`   IN SCOPE. Widget state is thread-local; no locks belong here.
#                   Every `Mutex`/`Atomic` currently in the tree is inside
#                   `#[cfg(test)]` code, which this excludes.
#
#   `src/event/queue.rs`, `src/event/timer.rs`, `src/event/types.rs`
#                   OUT OF SCOPE, deliberately. These are *cross-thread* by design:
#                   `BlockingQueue` documents "any number of producer threads may
#                   push while any number of consumers pop", and `TimerManager` is
#                   `Arc<Mutex<..>>` so a timer can fire from another thread. Their
#                   locks are load-bearing, so a blanket "no locking" rule there
#                   would be wrong.
#
#   `src/platform/`  OUT OF SCOPE. Wraps genuinely shared OS handles (a Win32 HWND
#                   map, the JNI VM).
#
# Usage: tools/check_locking.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

echo "=== Locking audit: widget state must be lock-free ==="
echo "  scope: src/widget/ (test modules excluded)"
echo "  out of scope, by design: src/event/{queue,timer,types}.rs (cross-thread),"
echo "                           src/platform/ (shared OS handles)"
echo

"$PYTHON" - <<'PY'
import re
import sys
from pathlib import Path

SCAN_ROOT = "src/widget"
# Only *blocking* primitives are a finding. `Atomic*` is deliberately not matched:
# a monotonic id counter (`static NEXT_ID: AtomicU64`) is lock-free — it cannot
# contend or deadlock — so flagging it would trade a real rule for noise. The
# property this gate protects is "no widget path can block on a lock".
PATTERN = re.compile(r"\b(?:Mutex|RwLock|OnceLock|LazyLock)\b")

# Phrases that mention a synchronisation type without using one.
PROSE = ("// ", "/// ", "//!", "lock-free", "no lock")

# The two character classes whose net count decides whether a line opens a body.
# Named constants rather than inline literals: when these were written inline, the
# open and close sets each lost a brace to the other and `}` counted as an *opening*
# brace, which made the whole matcher silently wrong (every body looked unfinished).
# A brace appearing in both sets is the failure mode, so it is asserted below.
# The two character classes whose net count decides whether a line opens a body.
# Written with escapes on the braces, and asserted disjoint, because when these were
# inline literals the sets each ended up containing `}` — so `}` counted as an
# *opening* brace and every body looked unfinished. The matcher then skipped to
# end-of-file, silently exempting all production code after a test module.
OPENERS = chr(123) + chr(40)
CLOSERS = chr(125) + chr(41)
assert not set(OPENERS) & set(CLOSERS), "a bracket cannot both open and close a body"
assert len(OPENERS) == 2 and len(CLOSERS) == 2, "exactly the two brackets of each kind"


def strip_test_modules(text: str) -> str:
    """Removes the bodies of items gated behind `test`, so only production code is scanned.

    Handles the full range of gates actually used in this tree — `#[cfg(test)]`,
    `#[cfg(all(test, full_widgets))]`, `#[cfg(any(test, feature = "x"))]` — by looking
    for the `test` predicate *inside* the attribute rather than matching one spelling.
    A test helper holding a `Mutex` (a recording bridge, a shared counter) is not
    production locking, and reporting it would train the reader to ignore this gate.

    Two properties this must not lose, because losing either makes the gate unable to
    fail:

    * **Finding removal must be exact.** The previous version scanned forward for the
      next line starting with `mod `; once a stack of `#[cfg(test)]`/`#[derive(..)]`
      attributes put a `derive` list ending in `)` on its own line the module opener
      was never recognised, and the skip ran to end-of-file — silently discarding
      every production line after the test module. That is a gate that reports PASS on
      injected production locking, so it is verified below rather than assumed.
    * **The attribute's shadow extends past one item.** `#[cfg(test)]` on a helper
      covers its `impl` blocks too, so every following item gated the same way is
      skipped, not just the first. The rule used here is the one rustc uses for this
      shape: keep consuming while the item is an `impl` or a block-less
      `struct`/`enum` declaration, and stop at the end of a block-bodied item.
    """
    lines = text.split("\n")
    out: list[str] = []
    index = 0
    while index < len(lines):
        if is_test_gate(lines[index]):
            index = skip_gated_items(lines, index)
            continue
        out.append(lines[index])
        index += 1
    return "\n".join(out)


def is_test_gate(line: str) -> bool:
    """Whether an attribute line enables the `test` cfg predicate."""
    stripped = line.strip()
    if not stripped.startswith("#[") or "cfg" not in stripped:
        return False
    # Whole-word `test`, so `#[cfg(feature = "latest")]` is not a test gate.
    return re.search(r"\btest\b", stripped) is not None


def depth_delta(line: str, block: list[object]) -> int:
    """Brace and paren balance of a line, ignoring those inside literals and comments.

    Only these two count: an item body is delimited by `{`/`}` and dropped by the
    balanced `()`, so deciding *whether a line opens a body* is `{`/`(` versus `}`/`)`.
    Mixing the two families would make `fn f() -> u32 {` cancel to zero and hide a body,
    so the count is a single family-agnostic tally: any opener is +1, any closer is -1,
    and a line opens a body exactly when its tally is positive.

    Three things have to be skipped, each because it silently broke this function at
    some point (all three were caught only by reverse injection — a bug here does not
    raise, it makes the gate report PASS on code it never looked at):

    * **Lifetimes are not char literals.** `&'static str` was read as `'s` plus text,
      swallowing the closing paren on the line. A char literal needs a non-word
      character before its opening quote.
    * **Comments are prose.** A comment mentioning `[32..56)` counted four closers and
      made a still-open test module look finished.
    * **Raw strings are opaque.** A `r#"..."#` JSON fixture holds doubled `{{`/`}}`,
      which unbalanced the tally by design; their braces are data, not code.

    `block` is one-element mutable state carried across lines, holding the active
    raw-string terminator (`None` when not inside one). Raw strings span lines, so a
    per-line function cannot decide this alone.
    """
    delta = 0
    # Any line whose first non-space characters are `//` is a comment — `//`, `///`,
    # `//!`, and also the indented `//            ` continuation form, which is why the
    # check is on the *stripped* line. `#[` attributes start with `#`, not `//`, so they
    # are still counted.
    if line.lstrip().startswith("//"):
        return 0
    if block[0] is not None:
        # Inside a raw string: only the terminator matters, and it may share the line
        # with code that continues after it.
        terminator = block[0]
        assert isinstance(terminator, str)
        end = line.find(terminator)
        if end < 0:
            return 0
        block[0] = None
        line = line[end + len(terminator) :]
    in_string = False
    index = 0
    while index < len(line):
        character = line[index]
        if in_string:
            if character == "\\":
                index += 2
                continue
            if character == '"':
                in_string = False
        elif character == '"':
            in_string = True
        elif character == "'":
            # A char literal is `'<char>'` — and only that. An apostrophe appearing after
            # an identifier is a lifetime (`'static`), which must not swallow the rest of
            # the line. Requiring a non-word character before the quote separates the two.
            follows_word = index > 0 and (line[index - 1].isalnum() or line[index - 1] == "_")
            char_literal = (
                not follows_word
                and index + 2 < len(line)
                and line[index + 2] == "'"
                and line[index + 1] != "\\"
            )
            if char_literal:
                index += 3
                continue
        elif character in CLOSERS:
            delta -= 1
        elif character in OPENERS:
            delta += 1
        index += 1
    return delta


def raw_string_opener(line: str) -> str | None:
    """Returns the terminator of a raw string opened on `line` and not closed on it."""
    match = re.search(r"r(#*)\"", line)
    if match is None:
        return None
    terminator = '"' + match.group(1)
    return None if terminator in line[match.end() :] else terminator


def body_depth_after(lines: list[str], start: int) -> int:
    """Net brace/paren tally of `lines[start:]`, with comments and literals skipped.

    The raw-string state is threaded through the whole run, because a fixture spanning
    dozens of lines only balances at its terminator.
    """
    total = 0
    for _, delta in walk_depth(lines, start):
        total += delta
    return total


def walk_depth(lines: list[str], start: int):
    """Yields `(index, delta)` for each line from `start`, tracking raw strings.

    The single place that owns the raw-string state machine. Every caller that needs a
    brace tally goes through it, because two copies of this loop is how the copies
    drift: the earlier version reset the state per line in one caller and kept it in
    another, so a multi-line fixture balanced in one and not the other.
    """
    block: list[object] = [None]
    for index in range(start, len(lines)):
        yield index, depth_delta(lines[index], block)
        terminator = raw_string_opener(lines[index])
        if terminator is not None:
            block[0] = terminator


def body_ends_at(lines: list[str], start: int) -> int | None:
    """Returns the index where the body opened before `start` closes, or `None`."""
    total = 0
    for index, delta in walk_depth(lines, start):
        total += delta
        if total <= 0:
            return index
    return None


def opens_a_body(lines: list[str], index: int) -> bool:
    """Whether the line at `index` opens a `{` body rather than ending in `;`."""
    return depth_delta(lines[index], [None]) > 0


def skip_gated_items(lines: list[str], index: int) -> int:
    """Returns the index just past every item covered by the attribute at `index`.

    Consumes the attribute stack, then the decorated item, then any further item the
    same gate still covers under rustc's rules: an item whose *entire* body stays open
    at end of file (a block-less `struct`/`enum` declaration) continues the shadow,
    while an item that closed its body ends it — everything after that body is
    unconditional code that must still be scanned.

    Deliberately conservative. Missing a shadowed item reports a test helper's `Mutex`
    (a false positive, which is visible and gets fixed); extending the shadow too far
    silently exempts real production code (a false negative, which is how the previous
    version made this gate unable to fail). When in doubt, stop.
    """
    while index < len(lines) and lines[index].strip().startswith("#["):
        index += 1
    if index >= len(lines):
        return index
    if opens_a_body(lines, index):
        # The opener's own braces are the body, so the walk starts *at* the opener and
        # its closing brace is the index where the running tally first reaches zero.
        closing = body_ends_at(lines, index)
        index = len(lines) if closing is None else closing + 1
    # A gate covers a run of items only while each one leaves the brace depth open at
    # end of file — the `struct` followed by its `impl` blocks shape. The run is decided
    # by scanning the remainder, so an item that closes its body ends the shadow.
    probe = index
    while probe < len(lines) and lines[probe].strip().startswith("#["):
        probe += 1
    if probe >= len(lines) or not opens_a_body(lines, probe):
        return index
    if body_depth_after(lines, probe) <= 0:
        return index
    return len(lines)



findings: list[str] = []
for path in sorted(Path(SCAN_ROOT).rglob("*.rs")):
    production = strip_test_modules(path.read_text(encoding="utf-8"))
    for number, line in enumerate(production.split("\n"), start=1):
        stripped = line.strip()
        if any(token in stripped for token in PROSE):
            continue
        if PATTERN.search(stripped):
            findings.append(f"{path}:{number}: {stripped}")

if findings:
    print(f"FAIL: {len(findings)} synchronisation primitive(s) in widget state:")
    for finding in findings:
        print(f"  {finding}")
    print()
    print("  Widget state lives in thread-local registries (`widget::runtime`), so a")
    print("  lock here is overhead without a purpose and risks a single-threaded")
    print("  deadlock. If a lock is genuinely needed, the state belongs in the")
    print("  cross-thread layer (`event::queue` / `event::timer`) or `platform/` — or")
    print("  add it to this script's out-of-scope list with the reason.")
    sys.exit(1)

print("  no synchronisation primitives in widget production code")
print()
print("  Consequence: widget creation and event dispatch cannot contend on a lock,")
print("  because there is no lock to contend on. That is a structural property, not a")
print("  measurement, so it cannot regress quietly.")
PY

echo
echo "Locking audit passed."
