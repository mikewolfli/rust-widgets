"""Self-test for the `depth_delta`/`skip_gated_items` helpers in `tools/check_locking.sh`.

The locking gate's whole value is that it can *fail*. Its analysis runs in Python
embedded in the shell script, so nothing else exercises it, and a bug in the
brace-matcher does not surface as an error — it surfaces as a silent PASS. That is
what happened: a lifetime (`&'static str`) was read as a char literal, the char
literal swallowed the closing paren of the line, and from there every subsequent
line read as opening a body. `skip_gated_items` therefore missed the test module's
closing brace and skipped to end-of-file, so a `Mutex` injected *after* the test
module was never reported. These cases pin the behaviour that fix depends on.

Usage: tools/test_check_locking.py   (exit 0 = pass, 1 = failure)
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "tools" / "check_locking.sh"

# The gate's Python is the region between these anchors. Extracting it keeps this
# test honest: it exercises the shipped code, not a copy that can drift from it.
source = SCRIPT.read_text(encoding="utf-8")
start = source.index("PATTERN = re.compile")
end = source.index("findings: list[str] = []")
namespace: dict = {"re": re}
exec(compile(source[start:end], "check_locking.sh:python", "exec"), namespace)
depth_delta = namespace["depth_delta"]
strip_test_modules = namespace["strip_test_modules"]
is_test_gate = namespace["is_test_gate"]

failures: list[str] = []


def expect(label: str, got: object, want: object) -> None:
    if got != want:
        failures.append(f"{label}: got {got!r}, want {want!r}")


# ── depth_delta: a line "opens a body" exactly when its tally is positive ──────
DEPTH_CASES = [
    # (line, expected tally)
    ("mod tests {", 1),
    ("fn f() -> u32 {", 1),
    ("    fn calls(&self) -> Vec<(&str, X)> {", 1),
    # A body opens and closes on one line: balanced, so it does not open a body.
    ("fn commit_text(&self, _text: &str) {}", 0),
    ("if x { \"a)b\" }", 0),
    ("", 0),
    ("static S: T = T::new(0);", 0),
    # Lifetimes are code, not char literals. This is the regression that made the
    # gate blind: `'static` used to swallow the rest of the line.
    ("calls: crate::compat::Mutex<Vec<(&'static str, ObjectId)>>,", 0),
    ("fn focus_in(&self, widget_id: ObjectId) {", 1),
    # Real char literals are skipped, including an escaped quote.
    ("let c = '}';", 0),
    ("let c = '\\'';", 0),
    ("let s = \"}\";", 0),
    # Doc comments are prose; braces in them must not move the tally.
    ("/// a `{` and a `}` mentioned in prose:", 0),
    ("//! module doc with }", 0),
    ("// a trailing brace }", 0),
    # Attributes are code and must still be counted.
    ("#[derive(Default)]", 0),
    ("#[cfg(all(test, full_widgets))]", 0),
]
for line, expected in DEPTH_CASES:
    expect(f"depth_delta({line!r})", depth_delta(line, [None]), expected)

# ── depth_delta: raw strings are opaque, across lines ────────────────────────
# A `r#"..."#` JSON fixture is full of doubled `{{`/`}}`. Counting them unbalanced the
# tally and made a still-open test module look closed.
RAW_OPENS = ('let json = r#"{{', '"op":30,"fr":30', '}}"#;')
block: list[object] = [None]
tallies = []
for raw_line in RAW_OPENS:
    tallies.append(depth_delta(raw_line, block))
    terminator = namespace["raw_string_opener"](raw_line)
    if terminator is not None:
        block[0] = terminator
expect("raw string body is opaque", tallies, [0, 0, 0])
expect("raw string closed", block[0], None)

# ── is_test_gate: whole-word `test` inside a cfg attribute ────────────────────
GATE_CASES = [
    ("#[cfg(test)]", True),
    ("#[cfg(all(test, full_widgets))]", True),
    ("#[cfg(any(test, feature = \"x\"))]", True),
    ("#[cfg(feature = \"latest\")]", False),
    ("#[cfg(feature = \"desktop\")]", False),
    ("#[derive(Default)]", False),
    ("mod tests {", False),
]
for line, expected in GATE_CASES:
    expect(f"is_test_gate({line!r})", is_test_gate(line), expected)

# ── strip_test_modules: exact removal, nothing beyond ────────────────────────
SAMPLE = "\n".join(
    [
        "pub fn production_before() {}",
        "",
        "#[cfg(all(test, full_widgets))]",
        "mod tests {",
        "    #[derive(Default)]",
        "    struct Recording {",
        "        calls: crate::compat::Mutex<Vec<(&'static str, ObjectId)>>,",
        "    }",
        "",
        "    impl Recording {",
        "        fn calls(&self) -> Vec<(&'static str, ObjectId)> {",
        "            self.calls.lock().unwrap().clone()",
        "        }",
        "    }",
        "}",
        "",
        "pub fn production_after() {}",
    ]
)
stripped = strip_test_modules(SAMPLE)
expect("strips the test module", "Mutex" in stripped, False)
expect("keeps code before the gate", "production_before" in stripped, True)
expect("keeps code after the gate", "production_after" in stripped, True)

# The rule that bit us: an unconditional item after a gated one must survive.
SAMPLE_TWO_ITEMS = "\n".join(
    [
        "#[cfg(test)]",
        "fn helper() {}",
        "",
        "#[cfg(test)]",
        "struct Widget {",
        "    id: u32,",
        "}",
        "",
        "#[cfg(test)]",
        "impl Widget {",
        "    fn id(&self) -> u32 {",
        "        self.id",
        "    }",
        "}",
        "",
        "pub fn after() {}",
    ]
)
stripped_two = strip_test_modules(SAMPLE_TWO_ITEMS)
expect("strips a multi-item gated group", "Widget" in stripped_two, False)
expect("keeps the unconditional item", "after" in stripped_two, True)

# ── Real files: the gate must keep every production line after a test module ──
# Verified against the tree rather than a fixture, so a new module layout that
# defeats the matcher is caught here.
PRODUCTION_LOCK_PATTERN = re.compile(r"\b(?:Mutex|RwLock|OnceLock|LazyLock)\b")
for path in sorted((ROOT / "src" / "widget").rglob("*.rs")):
    production = strip_test_modules(path.read_text(encoding="utf-8"))
    for number, line in enumerate(production.split("\n"), start=1):
        if PRODUCTION_LOCK_PATTERN.search(line):
            failures.append(f"{path.relative_to(ROOT)}:{number}: unreported lock: {line.strip()}")

if failures:
    print(f"FAIL: {len(failures)} case(s):")
    for failure in failures:
        print(f"  {failure}")
    sys.exit(1)

print("check_locking self-test: all cases pass")
