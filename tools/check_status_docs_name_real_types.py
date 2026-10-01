#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""A backend's `status.md` may not claim native objects the backend's source does not name.

# Why this gate exists

`src/platform/*/status.md` is what a host reads before choosing a build — it is the page that says
"this backend creates real `NSWindow`/`NSButton`/… objects" or "this backend is state-only". No gate
read it. The consequence, found by hand in one round:

* `src/platform/macos/status.md` claimed, for **both** macOS backends, per-kind `create_*` controls
  ("real `NSWindow`/`NSButton`/`NSTextField`/`NSPopover`…"), an `NSMenu` menu bar "installed as
  `NSApplication.mainMenu`", and `NSAlert`/`NSOpenPanel`/`NSColorPanel`/`NSFontPanel` dialogs.
* **None of it was in the code.** `grep -rn "NSButton|NSTextField|NSAlert|NSOpenPanel"
  src/platform/macos/` matched only that file's own prose, and `grep -c "fn create_"
  platform_impl.rs` returned **1** (`create_window`). BLUE15 #55/#56 had removed per-kind control
  creation and the table was simply never updated.

That is the "documentation describes behaviour the code does not have" defect (principle #18), and
it is the same shape as the capability flags: a claim that nothing checks, drifting silently.

# What is checked

For each `status.md` under `src/platform/`, every **native type name** it mentions in a ✅ row is
looked for in that backend's own sources. A name that appears nowhere in the code, in a row that
claims the capability, is a claim about an object this backend does not create.

A fourth check covers a Java/Kotlin **sample**: if a fenced block declares `package
io.github.rustwidgets` or `package rust_widgets`, every `native*` name inside it must be declared
by that wrapper's class. The two wrappers share the name `RustWidgets` and do not share their
methods, so the `package` line is the only thing that identifies which one a sample is quoting —
see `JAVA_WRAPPERS` for the defect that made this necessary.

# Why the check is this narrow, and what it deliberately does not attempt

The hard part is not the comparison, it is knowing which prose is a *claim*. A status page also
legitimately names a type to say it is **absent** ("⛔ Deleted — the library paints them"), or to
describe **history** ("the backend used to instantiate a real `UIView` per logical widget"). Those
are true statements about types the code does not name, so a naive "the name must appear in the
source" rule reports them. Two things keep the rule honest:

* the scan runs over **✅ rows only** — a cell that does not claim a capability is not a claim;
* a row is **skipped** when the same line, or the note column, carries a negation marker
  (`⛔`, `Not implemented`, `Deleted`, `Removed`, `used to`, `no longer`, `not bound`).

Both are heuristic. The rule is therefore a **linter with deliberate false negatives**, not a proof:
it catches the shape it was written for (a ✅ row naming a native type the backend never mentions)
and stays quiet on negation and history. A finding is a prompt to look, not a verdict — the module
doc of `check_capability_flags_match_their_methods.py` makes the same admission about its own
`dpi_scaling` rule, which was removed for exactly this reason.

Usage: python3 tools/check_status_docs_name_real_types.py
"""

from __future__ import annotations

import pathlib
import re
import sys

# The native types whose presence in a ✅ row is a claim that the backend creates one.
#
# Grouped by the backend whose sources are searched, because `NSAlert` in android's status.md would
# be a different (and stranger) claim than `NSAlert` in macos's.
NATIVE_TYPES: dict[str, tuple[str, ...]] = {
    "macos": (
        "NSWindow",
        "NSButton",
        "NSTextField",
        "NSTextView",
        "NSPopover",
        "NSMenu",
        "NSMenuItem",
        "NSAlert",
        "NSOpenPanel",
        "NSColorPanel",
        "NSFontPanel",
        "NSPasteboard",
    ),
    "macos_objc2": (
        "NSWindow",
        "NSButton",
        "NSTextField",
        "NSMenu",
        "NSMenuItem",
        "NSAlert",
        "NSOpenPanel",
        "NSPasteboard",
    ),
    "ios": ("UIWindow", "UIButton", "UILabel", "UIView", "UITextField", "UITableView"),
    "android": ("AlertDialog", "FrameLayout", "SeekBar", "ProgressBar"),
    # Harmony's real native types are `OH_NativeXComponent` and the ArkUI *C* headers. `ArkUI`
    # alone is a product name that appears in the backend's own prose, so listing it would make the
    # gate unfalsifiable — it would find its own subject in a comment and pass. Only the bound
    # type names are listed, which is the same discipline the other groups follow.
    "harmony": ("OH_NativeXComponent", "OH_NativeWindow", "napi_env", "ArkUI_NativeNodeAPI_1"),
}

# `status.md`-style documents checked, and the roots whose sources are searched for the names they
# claim. Android and iOS need more than their own directory: their native objects are created on the
# **binding** side (the JNI entry points that construct the Java-side views), so a name that appears
# only in `src/bindings/java_jni.rs` or under `bindings/android/` is genuinely bound. A first version
# of this gate searched only `src/platform/android/` and reported four Android rows as overclaims
# when the symbols were exported from the bindings — a false positive that would have led to
# "correcting" a truthful status page.
DOCUMENTS: tuple[tuple[str, str, tuple[str, ...]], ...] = (
    # `src/platform/macos/status.md` documents **both** macOS backends in one page, so each must be
    # checked against both source trees: a row under the "objc2 backend" heading that names
    # `NSPasteboard` is true if either backend binds it, and attributing every row to whichever
    # backend happens to be named in this tuple reported two of them as overclaims.
    ("macos", "src/platform/macos/status.md", ("src/platform/macos", "src/platform/macos_objc2")),
    ("macos_objc2", "src/platform/macos/status.md", ("src/platform/macos", "src/platform/macos_objc2")),
    ("ios", "src/platform/ios/status.md", ("src/platform/ios",)),
    (
        "android",
        "src/platform/android/status.md",
        ("src/platform/android", "src/platform/android_jni.rs", "src/bindings/java_jni.rs"),
    ),
    (
        "android",
        "src/platform/android/activity_integration.md",
        ("src/platform/android", "src/platform/android_jni.rs", "src/bindings/java_jni.rs"),
    ),
    ("harmony", "src/platform/harmony/status.md", ("src/platform/harmony",)),
)

# JNI entry-point names: a doc naming one of these is claiming an export, and whether it exists is a
# `grep` away in `src/bindings/java_jni.rs`. This is the mechanically checkable half of the same
# defect class as the native type names — `activity_integration.md` named `nativeCreateSeekBar` and
# an entire `nativeSetView*` family that were never written.
JNI_ENTRY_RE = re.compile(r"\bnative(?:Create|SetView)[A-Za-z0-9_]+")
JNI_EXPORT_SOURCE = "src/bindings/java_jni.rs"

# A row that is not claiming a capability.
#
# The list starts with the panel's own **negative** status symbols, because a row whose status cell
# says "not implemented" is a true statement about a type the code does not name — reporting it would
# be a finding about a page that is already honest.
NEGATION_MARKERS = (
    "⛔",
    "⬜",
    "❌",
    "Not implemented",
    "not implemented",
    "Deleted",
    "Removed",
    "used to",
    "no longer",
    "not bound",
    "None of that",
    "does not",
    "do not",
    "never",
)

# OS **widget** classes that must never appear as an achieved capability in a platform document.
#
# # Why this is a rule rather than a wording preference
#
# The library paints **every** `WidgetKind` itself, on **every** backend — there are no native
# per-kind controls and there have not been since BLUE15 #55/#56/#59. A document listing
# `android.widget.Button` / `UIButton` / `NSButton` / `gtk::Notebook` beside a ✅ teaches a reader the
# opposite, and that is not cosmetic: a host reads these pages to decide what the library will do.
#
# # Why the list is closed, and what it deliberately excludes
#
# The first version of this rule matched `NS[A-Z]*` / `UI[A-Z]*`, which flagged `NSWindow`,
# `NSApplication`, `NSPasteboard` and `UIWindow` — the **window, event loop and platform services**
# the library legitimately uses on those hosts. Those are not per-kind controls; clipboard and
# window creation are real host integrations that survive precisely because the library paints *into*
# a host window. Flagging them would have demanded that four true statements be deleted, which is the
# failure mode where a gate reports the spelling of a fix rather than the fix.
#
# So the rule names only the classes that correspond to a `WidgetKind` the library paints. A class not
# on this list is not checked, and that is intentional: the list is a statement of the claim being
# forbidden, not a catalogue of every OS type.
NATIVE_WIDGET_CLASS_RE = re.compile(
    r"\b(?:android\.widget\.(?:Button|TextView|EditText|CheckBox|RadioButton|SeekBar|ProgressBar"
    r"|Spinner|ListView|ScrollView|NumberPicker|FrameLayout|ImageView|Switch|ToggleButton|RatingBar)"
    r"|android\.app\.AlertDialog"
    r"|UI(?:Button|Label|Switch|TextField|PickerView|TableView|ProgressView|Slider|ScrollView"
    r"|StackView|ActivityIndicatorView)"
    r"|NS(?:Button|TextField|TextView|Popover|Menu|MenuItem|Alert|OpenPanel|ColorPanel|FontPanel)"
    r"|gtk::(?:Button|Label|Entry|CheckButton|ComboBoxText|ListBox|ProgressBar|Scale|SpinButton"
    r"|Notebook|Frame|Paned|Calendar|Menu|Window))\b"
)

# Markers that make a native widget mention legitimate: it is describing the deletion, not a
# capability. Deliberately broader than `NEGATION_MARKERS` because a *✅ row* here can still be
# legitimate when it is recording that a class is no longer used.
HISTORICAL_MARKERS = NEGATION_MARKERS + (
    "former",
    "Former",
    "never instantiated",
    "deleted",
    "deletion",
    "gone",
    # A row that names a class in order to deny it. `"no `android.app.AlertDialog` is
    # constructed"` is a true statement about the deletion, and this is the phrasing the corrected
    # tables use; without it the rule reports the correction as the defect.
    "is constructed",
    "is created",
    "is instantiated",
    "exists",
)


# A `| … | ✅ … |` row, but not the header or the separator.
ROW_RE = re.compile(r"^\|\s*(?P<area>[^|]+?)\s*\|\s*(?P<status>[^|]+?)\s*\|(?P<note>.*)\|\s*$")


def source_text(backend: str, roots: tuple[str, ...]) -> str:
    """The backend's sources with **comments stripped**.

    # Why comments are removed before the search

    A backend's rationale comments name the exact types it *does not* construct — that is how a
    reader is told why a capability is absent. Counting those as evidence that the type is
    implemented would make the gate pass on the strength of the prose explaining the gap, which is
    the opposite of what it is for. Only code counts as evidence here.

    `roots` may name a directory (walked for `*.rs`) or a single file.
    """
    parts = []
    for entry in roots:
        path = pathlib.Path(entry)
        if not path.exists():
            continue
        files = sorted(path.rglob("*.rs")) if path.is_dir() else [path]
        for file in files:
            for line in file.read_text(encoding="utf-8").splitlines():
                stripped = line.lstrip()
                # Drop whole-line comments (`//`, `///`, `//!`) and split off trailing ones, so a
                # mention inside either kind is not read as an implementation.
                if stripped.startswith("//"):
                    continue
                parts.append(line.split("//", 1)[0])
    return "\n".join(parts)


def check_status_doc(backend: str, status_path: pathlib.Path, roots: tuple[str, ...]) -> list[str]:
    findings: list[str] = []
    if not status_path.exists():
        return findings
    text = source_text(backend, roots)
    if not text:
        return findings

    for lineno, line in enumerate(status_path.read_text(encoding="utf-8").splitlines(), start=1):
        m = ROW_RE.match(line)
        if not m:
            continue
        status = m.group("status")

        # # The JNI check runs on every row; the native-type check only on ✅ rows
        #
        # The two are not the same question. A native type name is a claim only inside a ✅ row,
        # because the same word appears legitimately in a `⛔ Deleted` row. A **JNI symbol name** is
        # a claim wherever it appears: `nativeCreateSeekBar` is either an entry point this crate
        # exports or it is not, and a document that names it in any status cell is pointing a reader
        # at something that does not exist.
        #
        # Scoping this to ✅ rows was the first version, and it was wrong in exactly the way this
        # gate exists to prevent: the reproduction of `activity_integration.md`'s original
        # overclaim below is a row whose status cell is not a check mark, so the check silently
        # skipped the defect it was written for.
        # # Why this is skipped on rows that already say the symbol is absent
        #
        # A row whose status is `⛔ No Rust export` names the symbols precisely to record that they
        # do not exist, and the `grep` command in its note contains them by construction. Reporting
        # that as a false claim would be reporting the *documentation of the gap* as the gap — the
        # same error as reading a backend's rationale comment as evidence of what it implements.
        #
        # The reproduction of the original overclaim (a Spring/✅ row) still fires, because it is a
        # row that does not carry one of these markers.
        if any(marker in line for marker in NEGATION_MARKERS):
            continue
        for entry_name in set(JNI_ENTRY_RE.findall(line)):
            if entry_name in declared_jni_entry_points():
                continue
            findings.append(
                f"{status_path.as_posix()}:{lineno} names the JNI entry point `{entry_name}`, "
                f"which is not declared in {JNI_EXPORT_SOURCE} — the document points a reader at a "
                f"binding that was never written"
            )

        if "✅" not in status:
            # Not a capability claim.
            continue

        # No OS widget class may appear as an achieved capability. See `NATIVE_WIDGET_CLASS_RE`.
        if not any(marker in line for marker in HISTORICAL_MARKERS):
            for cls in set(NATIVE_WIDGET_CLASS_RE.findall(line)):
                findings.append(
                    f"{status_path.as_posix()}:{lineno} names the OS widget class `{cls}` in a ✅ "
                    f"row. This library paints every `WidgetKind` itself on every backend — there "
                    f"are no native per-kind controls — so the row teaches a reader a mapping that "
                    f"does not exist"
                )

        # Both native-type and widget-class checks are skipped on a row that names the thing in
        # order to deny it — see `HISTORICAL_MARKERS` for why the marker set is broader here than
        # the one the JNI check uses (a symbol name is a claim wherever it appears; a *class* name
        # in a row that says "no `X` is constructed" is a true statement about the deletion).
        historical = any(marker in line for marker in HISTORICAL_MARKERS)
        if historical:
            continue
        for native in NATIVE_TYPES.get(backend, ()):
            if native not in m.group("area") and native not in m.group("note"):
                continue
            if native in text:
                continue
            findings.append(
                f"{status_path.as_posix()}:{lineno} claims `{native}` in a ✅ row, but that "
                f"identifier appears in none of {list(roots)} — the row describes "
                f"an object this backend does not create"
            )
    return findings


# The Java wrappers a document may be quoting, keyed by the **package** the sample declares.
#
# # Why the package, and not the class name
#
# Both wrappers are called `RustWidgets`, and they do not contain the same methods:
#
#   bindings/java/RustWidgets.java            package io.github.rustwidgets  → the desktop surface
#   bindings/android/java/rust_widgets/…      package rust_widgets           → the Android surface
#
# `src/platform/android/activity_integration.md` quoted the **desktop** wrapper's method list as
# an Android sample — while its own class-name note said the Android class is the one under
# scrutiny. A reader copying it would bind against names the Android class does not declare and
# get `UnsatisfiedLinkError` at the first call. The class name cannot distinguish the two, so the
# sample's `package` line is what says which wrapper it is describing.
JAVA_WRAPPERS: tuple[tuple[str, str], ...] = (
    ("io.github.rustwidgets", "bindings/java/RustWidgets.java"),
    ("rust_widgets", "bindings/android/java/rust_widgets/RustWidgets.java"),
)

# A `native*` method named in a Java/Kotlin sample. Wider than `JNI_ENTRY_RE` because a sample also
# calls lifecycle and diagnostic methods that are not per-kind creators.
SAMPLE_METHOD_RE = re.compile(r"\bnative[A-Za-z0-9_]+")


def declared_java_methods(path: str) -> set[str]:
    """The `native*` methods a Java wrapper class actually declares."""
    file = pathlib.Path(path)
    if not file.exists():
        return set()
    return set(re.findall(r"public static native\s+[\w.<>\[\]]+\s+(native[A-Za-z0-9_]+)", file.read_text(encoding="utf-8")))


def check_java_samples(status_path: pathlib.Path) -> list[str]:
    """Every `native*` name inside a fenced Java/Kotlin sample must be one the named wrapper has.

    A sample is identified by its fence language (`java`, `kotlin`) and attributed to a wrapper by
    the `package` line inside it. A sample with no `package` line is not attributed and is skipped,
    because guessing which wrapper it means is how the wrong answer gets written down.
    """
    lines = status_path.read_text(encoding="utf-8").splitlines()
    findings: list[str] = []
    fence: str | None = None
    start_line = 0
    body: list[str] = []
    for lineno, line in enumerate(lines, 1):
        stripped = line.strip()
        if fence is None:
            if stripped.startswith("```"):
                lang = stripped[3:].strip().lower()
                if lang in ("java", "kotlin"):
                    fence = lang
                    start_line = lineno
                    body = []
            continue
        if stripped.startswith("```"):
            fence = None
            package = None
            for body_line in body:
                m = re.match(r"\s*package\s+([\w.]+)", body_line)
                if m:
                    package = m.group(1)
                    break
            declared = dict(JAVA_WRAPPERS).get(package or "")
            if declared is not None:
                methods = declared_java_methods(declared)
                for body_line in body:
                    # A sample comment may name a method **to deny it belongs here** (""typed
                    # creators live in the *desktop* wrapper, not this class"" ). That is true
                    # history, and reporting it would be reporting the correction as the defect —
                    # the same rule `NEGATION_MARKERS` applies to the prose checks.
                    if body_line.lstrip().startswith(("//", "*", "/*")):
                        continue
                    for name in set(SAMPLE_METHOD_RE.findall(body_line)):
                        if name not in methods:
                            findings.append(
                                f"{status_path.as_posix()}:{start_line} sample (package {package}) "
                                f"calls `{name}`, which {declared} does not declare"
                            )
            continue
        body.append(line)
    return findings


def declared_jni_entry_points() -> set[str]:
    """The `native*` JNI entry points actually declared in `src/bindings/java_jni.rs`.

    The declarations are the `Java_io_github_rustwidgets_RustWidgets_<name>,` first argument of each
    `jni_create_*!` macro, so the symbol name is the segment after the last underscore-delimited
    class prefix. Reading them from the source rather than a list keeps the check anchored to the
    one file `tools/check_jni_signatures.sh` already validates.
    """
    path = pathlib.Path(JNI_EXPORT_SOURCE)
    if not path.exists():
        return set()
    text = path.read_text(encoding="utf-8")
    prefix = "Java_io_github_rustwidgets_RustWidgets_"
    return {
        m.group(1)
        for m in re.finditer(re.escape(prefix) + r"([A-Za-z0-9_]+)", text)
    }


def inject_native_widget_row() -> int:
    """Insert a `✅ UIButton` row and require the gate to notice the native-control framing.

    This is the injection for the rule the user's correction added: the library paints every
    `WidgetKind`, so a ✅ row pairing a logical kind with an OS widget class is a false claim about
    the architecture — and it is the specific shape all three hand-corrections had to remove.
    """
    path = pathlib.Path("src/platform/ios/status.md")
    original = path.read_text(encoding="utf-8")
    row = "\n| Button | ✅ Implemented | real `UIButton` per control |\n"
    if "real `UIButton` per control" in original:
        print("❌ the injection row already exists; it was supposed to be corrected")
        return 1
    try:
        path.write_text(original + row, encoding="utf-8")
        found = check_status_doc("ios", path, ("src/platform/ios",))
    finally:
        path.write_text(original, encoding="utf-8")
    if any("OS widget class" in f for f in found):
        print("✅ reverse injection: a ✅ row naming an OS widget class is detected")
        return 0
    print("❌ reverse injection: the native-control framing was NOT detected")
    return 1


def inject_jni() -> int:
    """Restore the `nativeCreateSeekBar` claim and require the gate to notice.

    A second injection because the two checks inside this gate answer different questions and the
    first version of the JNI one was **scoped wrongly** (to ✅ rows only), so it skipped exactly the
    defect it was written for. Proving each half independently is what caught that; a single
    injection covering only the native-type half would have left the JNI half silently dead.
    """
    path = pathlib.Path("src/platform/android/activity_integration.md")
    original = path.read_text(encoding="utf-8")
    row = "\n| Widget | ✅ Implemented | `nativeCreateSeekBar` | — |\n"
    if "| Widget | ✅ Implemented | `nativeCreateSeekBar`" in original:
        print("❌ the injection row already exists; it was supposed to be corrected")
        return 1
    try:
        path.write_text(original + row, encoding="utf-8")
        found = check_status_doc(
            "android",
            path,
            ("src/platform/android", "src/platform/android_jni.rs", "src/bindings/java_jni.rs"),
        )
    finally:
        path.write_text(original, encoding="utf-8")
    if found:
        print("✅ reverse injection: a document naming an unexported JNI entry point is detected")
        return 0
    print("❌ reverse injection: the unexported JNI entry point was NOT detected")
    return 1


def inject() -> int:
    """Restore the macOS overclaim and require the gate to notice."""
    path = pathlib.Path("src/platform/macos/status.md")
    original = path.read_text(encoding="utf-8")
    # Put back one of the rows the correction removed.
    marker = "| `create_window` | ✅ Implemented | a real `NSWindow` on the main thread |"
    replacement = (
        marker + "\n| Menu bar (`NSMenu` + `NSMenuItem`) | ✅ Verified | installed as "
        "`NSApplication.mainMenu` |"
    )
    if marker not in original:
        print("❌ injection point not found in src/platform/macos/status.md")
        return 1
    try:
        path.write_text(original.replace(marker, replacement, 1), encoding="utf-8")
        found = check_status_doc("macos", path, ("src/platform/macos",))
    finally:
        path.write_text(original, encoding="utf-8")
    if found:
        print("✅ reverse injection: a ✅ row naming an object the backend never creates is detected")
        return 0
    print("❌ reverse injection: the overclaim was NOT detected")
    return 1


def inject_java_sample() -> int:
    """Add a method the Android wrapper does not declare and require the gate to notice.

    This is the injection for the fourth check: a sample's `package` line attributes it to one of
    two same-named wrapper classes, and the name it calls has to exist in *that* class. The defect
    it was written for listed the desktop wrapper's methods under the Android wrapper's package.
    """
    path = pathlib.Path("src/platform/android/activity_integration.md")
    original = path.read_text(encoding="utf-8")
    marker = "    external fun nativeInstallLogging()"
    if marker not in original:
        print("❌ injection point not found in src/platform/android/activity_integration.md")
        return 1
    try:
        path.write_text(
            original.replace(marker, marker + "\n    external fun nativeCreateSeekBar(id: Long): Long", 1),
            encoding="utf-8",
        )
        found = check_java_samples(path)
    finally:
        path.write_text(original, encoding="utf-8")
    if found:
        print("✅ reverse injection: a sample calling a method its named wrapper lacks is detected")
        return 0
    print("❌ reverse injection: the wrong-wrapper sample was NOT detected")
    return 1


def main() -> int:
    if "--inject" in sys.argv:
        return inject() | inject_native_widget_row() | inject_jni() | inject_java_sample()

    findings: list[str] = []
    checked = 0
    visited = 0
    for backend, doc_path, roots in DOCUMENTS:
        status_path = pathlib.Path(doc_path)
        if not status_path.exists():
            findings.append(f"{doc_path} is listed but does not exist; the file moved")
            continue
        visited += 1
        checked += len(NATIVE_TYPES.get(backend, ()))
        findings += check_status_doc(backend, status_path, roots)
        findings += check_java_samples(status_path)

    print(f"platform docs: {checked} native type name(s) checked across {visited} document(s); "
          f"{len(declared_jni_entry_points())} JNI entry point(s) derived from {JNI_EXPORT_SOURCE}")
    print()
    if findings:
        print(f"❌ a status page claims a native object its backend does not create "
              f"({len(findings)}):")
        for finding in findings:
            print(f"   {finding}")
        return 1
    print("✅ status docs: every ✅ row names objects the backend's sources mention")
    return 0


if __name__ == "__main__":
    sys.exit(main())
