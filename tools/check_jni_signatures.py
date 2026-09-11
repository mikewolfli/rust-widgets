#!/usr/bin/env python3
"""check_jni_signatures.py — Java ↔ Rust JNI signature contract gate.

Java `native` methods bind to Rust `#[no_mangle] pub extern "system"` functions
by *name mangling* plus the `(JNIEnv, JClass, ...)` parameter convention. A
mismatch in name, arity, or parameter type surfaces only at runtime as
`UnsatisfiedLinkError` (name/arity) or as corrupted stack reads (types), which
is why this check exists as a static gate.

What is validated:

  1. Every `native` method declared in a Java binding class (for the class whose
     mangled prefix matches the Rust module) has a corresponding Rust function
     of the same mangled name.
  2. Every Rust JNI function has a matching Java declaration (no orphan exports).
  3. Parameter types match positionally, after mapping Java types to the JNI C
     types the Rust side declares (`String -> JString`, `long -> jlong`,
     `int -> jint`, `boolean -> jboolean`).

The Java `Class.method` pair is converted to its JNI mangling:
`Java_<pkg with '.' -> '_'>_<Class>_<method>`, with `_` in the source name
escaped as `_1` (so the `io.github.rustwidgets` package becomes
`io_github_rustwidgets`).

Exit code is non-zero when any mismatch is found.

An optional `--report FILE` writes the full Java-method → JNI-symbol → Rust-export
mapping (with per-parameter types and library-export status) as JSON, so the
binding surface can be audited as data rather than only validated as a gate.

Usage:
    python3 tools/check_jni_signatures.py \
        --java bindings/java/RustWidgets.java \
        --java-class io.github.rustwidgets.RustWidgets \
        --rust src/bindings/java_jni.rs
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
from dataclasses import dataclass
from typing import Dict, List, Optional, Tuple

# Java primitive/reference type -> JNI C type used in the Rust signature.
JAVA_TO_JNI: Dict[str, str] = {
    "String": "JString",
    "long": "jlong",
    "int": "jint",
    "boolean": "jboolean",
    "byte[]": "jbyteArray",
    "double": "jdouble",
    "float": "jfloat",
    "short": "jshort",
    "char": "jchar",
}

# The two leading parameters every JNI export receives.
JNI_PREFIX_PARAMS = ["JNIEnv", "JClass"]

# Java `native` declaration, possibly spanning multiple lines.
NATIVE_RE = re.compile(
    r"(?:public|private|protected)?\s*static\s+native\s+"
    r"(?P<ret>[\w\[\]<>., ]+?)\s+"
    r"(?P<name>\w+)\s*\((?P<params>[^;]*?)\)\s*;",
    re.DOTALL,
)

# Rust `pub extern "system" fn <name>( ... ) -> <ret> {`
RUST_FN_RE = re.compile(
    r"pub\s+extern\s+\"system\"\s+fn\s+(?P<name>Java_\w+)\s*(?:<[^>]*>)?\s*\((?P<params>[^)]*)\)"
    r"(?:\s*->\s*(?P<ret>[\w<>'_: ]+))?",
    re.DOTALL,
)


@dataclass(frozen=True)
class JavaNative:
    name: str
    mangled: str
    return_type: str
    params: List[str]


def java_type_to_jni(t: str) -> Optional[str]:
    t = t.strip()
    if t in JAVA_TO_JNI:
        return JAVA_TO_JNI[t]
    # Any other reference type maps to an object handle; we only need the
    # leading JNI type family to compare against the Rust declaration.
    if t.endswith("[]"):
        return None
    return "JObject"


def mangle(package: str, class_name: str, method: str) -> str:
    """Build the JNI symbol name for a Java native method.

    Per the JNI spec, ``.`` separators become ``_``, and an underscore that is
    *part of* a source identifier is escaped as ``_1``. The escape therefore
    applies to each component (package segments, class, method) before they are
    joined, not to the joining separators.

    ``rust_widgets.RustWidgets.nativeInit``
      -> package ``rust_1widgets`` + ``RustWidgets`` + ``nativeInit``
      -> ``Java_rust_1widgets_RustWidgets_nativeInit``
    """

    def escape(component: str) -> str:
        return component.replace("_", "_1")

    package_part = "_".join(escape(seg) for seg in package.split("."))
    unified = f"{package_part}_{escape(class_name)}_{escape(method)}"
    return "Java_" + unified


def norm_type(t: str) -> str:
    """Normalise a Rust type token to its final path segment.

    The same JNI type may be written `jint` or `jni::sys::jint` depending on the
    module's imports; both mean the same ABI type, so compare on the last
    segment.
    """
    t = t.strip().split("<")[0].strip()
    return t.rsplit("::", 1)[-1]


def parse_java(path: pathlib.Path, package: str, class_name: str) -> Dict[str, JavaNative]:
    text = path.read_text(encoding="utf-8")
    # Strip block and line comments so commented-out declarations never bind.
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    text = re.sub(r"//[^\n]*", "", text)
    found: Dict[str, JavaNative] = {}
    for m in NATIVE_RE.finditer(text):
        raw_params = m.group("params").strip()
        params: List[str] = []
        if raw_params:
            for p in raw_params.split(","):
                # "long parent" -> "long"; handles "String title" etc.
                parts = p.strip().split()
                if len(parts) >= 2:
                    params.append(" ".join(parts[:-1]))
                elif parts:
                    params.append(parts[0])
        name = m.group("name")
        found[name] = JavaNative(
            name=name,
            mangled=mangle(package, class_name, name),
            return_type=m.group("ret").strip(),
            params=params,
        )
    return found


def strip_rust_comments(text: str) -> str:
    """Remove Rust line and block comments, respecting string literals.

    A naive regex strip is wrong: `"*/*"` (a MIME wildcard) contains `/*`, which
    a regex treats as the start of a block comment and swallows source until the
    next `*/`, hiding real exports from the scan. This scanner tracks whether it
    is inside a `"..."` / `'...'` literal (with backslash escapes and raw
    strings) so comment markers inside strings are left alone, while preserving
    newlines so reported offsets stay meaningful.
    """
    out: List[str] = []
    i = 0
    n = len(text)
    while i < n:
        ch = text[i]

        # Raw string: r"...", r#"..."#, r##"..."## …
        if ch in "rb" and i + 1 < n:
            j = i + (1 if ch == "b" else 0)
            if j < n and text[j] == "r":
                k = j + 1
                hashes = 0
                while k < n and text[k] == "#":
                    hashes += 1
                    k += 1
                if k < n and text[k] == '"':
                    close = '"' + "#" * hashes
                    end = text.find(close, k + 1)
                    end = n if end == -1 else end + len(close)
                    out.append(text[i:end])
                    i = end
                    continue

        # Character literal or lifetime: 'a' vs 'a
        if ch == "'" and i + 2 < n:
            if text[i + 2] == "'":
                out.append(text[i : i + 3])
                i += 3
                continue

        # String literal (possibly byte string).
        if ch == '"' or (ch == "b" and i + 1 < n and text[i + 1] == '"'):
            start = i
            if ch == "b":
                i += 1
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                    continue
                if text[i] == '"':
                    i += 1
                    break
                i += 1
            out.append(text[start:i])
            continue

        # Line comment.
        if text.startswith("//", i):
            end = text.find("\n", i)
            if end == -1:
                break
            out.append("\n")
            i = end + 1
            continue

        # Block comment (nesting is supported by Rust, so count depth).
        if text.startswith("/*", i):
            depth = 1
            i += 2
            while i < n and depth:
                if text.startswith("/*", i):
                    depth += 1
                    i += 2
                elif text.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    if text[i] == "\n":
                        out.append("\n")
                    i += 1
            continue

        out.append(ch)
        i += 1
    return "".join(out)


def parse_rust(path: pathlib.Path) -> Dict[str, Tuple[List[str], str]]:
    """Return mangled name -> (param type tokens, return type).

    Handles both literal `pub extern "system" fn` definitions and the three
    generation macros used in `java_jni.rs`
    (`jni_create_widget_with_text!`, `jni_create_widget_no_text!`,
    `jni_create_dialog!`), which emit the same shape with a `$name`.
    """
    text = strip_rust_comments(path.read_text(encoding="utf-8"))

    found: Dict[str, Tuple[List[str], str]] = {}
    for m in RUST_FN_RE.finditer(text):
        params: List[str] = []
        for p in m.group("params").split(","):
            # "mut env: JNIEnv<'\''>" -> type token after ':'
            if ":" in p:
                params.append(p.split(":", 1)[1].strip().split("<")[0])
        ret = (m.group("ret") or "").strip()
        found[m.group("name")] = (params, ret)

    found.update(parse_rust_macros(text))
    return found


# Macro definition: `macro_rules! <name> { ( $a:ident, $b:ident ) => { ... } }`
MACRO_DEF_RE = re.compile(
    r"macro_rules!\s+(?P<macro>\w+)\s*\{(?P<body>.*?)\n\}",
    re.DOTALL,
)
# Invocation: `macro_name!( JniSymbol, rw_fn );`
MACRO_CALL_RE = re.compile(r"(?P<macro>\w+)!\(\s*(?P<args>[^;]*?)\s*\)\s*;", re.DOTALL)
# Rust fn inside a macro body, with `$name` standing in for the symbol.
MACRO_FN_RE = re.compile(
    r"pub\s+extern\s+\"system\"\s+fn\s+\$name\s*\((?P<params>[^)]*)\)"
    r"(?:\s*->\s*(?P<ret>[\w<>'_: ]+))?",
    re.DOTALL,
)


def parse_rust_macros(text: str) -> Dict[str, Tuple[List[str], str]]:
    """Expand the known JNI generation macros into concrete signatures."""
    macro_bodies: Dict[str, Tuple[List[str], str]] = {}
    for m in MACRO_DEF_RE.finditer(text):
        fn = MACRO_FN_RE.search(m.group("body"))
        if not fn:
            continue
        params: List[str] = []
        for p in fn.group("params").split(","):
            if ":" in p:
                params.append(p.split(":", 1)[1].strip().split("<")[0])
        macro_bodies[m.group("macro")] = (params, (fn.group("ret") or "").strip())

    expanded: Dict[str, Tuple[List[str], str]] = {}
    for call in MACRO_CALL_RE.finditer(text):
        macro = call.group("macro")
        if macro not in macro_bodies:
            continue
        # First identifier argument is the exported symbol name.
        first = call.group("args").strip().split(",")[0].strip()
        if not first.startswith("Java_"):
            continue
        expanded[first] = macro_bodies[macro]
    return expanded


def validate(java: Dict[str, JavaNative], rust: Dict[str, Tuple[List[str], str]]) -> int:
    errors = 0

    for name, decl in sorted(java.items()):
        if decl.mangled not in rust:
            print(f"❌ Java native `{name}` has no Rust export `{decl.mangled}`")
            errors += 1
            continue
        rust_params, _rust_ret = rust[decl.mangled]
        # Skip the leading (JNIEnv, JClass) on the Rust side.
        body_params = rust_params[len(JNI_PREFIX_PARAMS):]
        expected = [java_type_to_jni(t) for t in decl.params]
        if len(body_params) != len(expected):
            print(
                f"❌ `{name}` arity mismatch: Java={len(expected)} "
                f"Rust={len(body_params)} ({decl.mangled})"
            )
            errors += 1
            continue
        for idx, (got, want) in enumerate(zip(body_params, expected)):
            if want is None:
                continue  # array type not modelled; arity already verified
            if norm_type(got) != want:
                print(
                    f"❌ `{name}` param #{idx}: Java implies {want}, "
                    f"Rust declares {got} ({decl.mangled})"
                )
                errors += 1

    orphan = set(rust) - {d.mangled for d in java.values()}
    for sym in sorted(orphan):
        print(f"❌ Rust export `{sym}` has no Java native declaration")
        errors += 1

    return errors


def read_exported_symbols(path: pathlib.Path) -> set:
    """Return the defined dynamic symbols from an ELF shared object.

    Uses `llvm-nm`/`nm` if available; returns an empty set when no symbol tool
    can be found so the check degrades to source-only rather than failing.
    """
    import shutil
    import subprocess

    nm = None
    for candidate in ("llvm-nm", "nm"):
        found = shutil.which(candidate)
        if found:
            nm = found
            break
    if nm is None:
        return set()
    try:
        out = subprocess.run(
            [nm, "-D", "--defined-only", str(path)],
            capture_output=True,
            text=True,
            check=False,
        ).stdout
    except OSError:
        return set()
    symbols = set()
    for line in out.splitlines():
        parts = line.split()
        if parts and parts[-1].startswith("Java_"):
            symbols.add(parts[-1])
    return symbols


def validate_symbols(java: Dict[str, JavaNative], symbols: set) -> int:
    """Check every Java declaration has a matching exported symbol."""
    if not symbols:
        return 0
    errors = 0
    for name, decl in sorted(java.items()):
        if decl.mangled not in symbols:
            print(f"❌ `{name}` not exported by the library as `{decl.mangled}`")
            errors += 1
    return errors


def build_report(
    java_path: pathlib.Path,
    java_class: str,
    rust_path: pathlib.Path,
    java: Dict[str, JavaNative],
    rust: Dict[str, Tuple[List[str], str]],
    symbols: set,
) -> dict:
    """Build the machine-readable Java↔Rust native method mapping report."""
    methods = []
    for name, decl in sorted(java.items()):
        rust_params, rust_ret = rust.get(decl.mangled, ([], ""))
        body_params = rust_params[len(JNI_PREFIX_PARAMS) :]
        methods.append(
            {
                "java_method": name,
                "java_return_type": decl.return_type,
                "java_params": list(decl.params),
                "jni_symbol": decl.mangled,
                "rust_declared": decl.mangled in rust,
                "rust_params": body_params,
                "rust_return_type": rust_ret,
                "exported_in_library": (decl.mangled in symbols) if symbols else None,
            }
        )
    return {
        "java_file": str(java_path),
        "java_class": java_class,
        "rust_file": str(rust_path),
        "java_declaration_count": len(java),
        "rust_export_count": len(rust),
        "exported_symbol_count": len(symbols) if symbols else 0,
        "methods": methods,
    }


def write_report(report: dict, output: pathlib.Path) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate Java↔Rust JNI signatures")
    parser.add_argument("--java", required=True, help="Java binding source file")
    parser.add_argument(
        "--java-class",
        required=True,
        help="Fully-qualified Java class holding the native declarations",
    )
    parser.add_argument("--rust", required=True, help="Rust JNI module source file")
    parser.add_argument(
        "--symbols",
        default=None,
        help="Optional built shared library to verify exported symbols against",
    )
    parser.add_argument(
        "--report",
        default=None,
        help="Optional path to write the Java↔Rust native method mapping as JSON",
    )
    args = parser.parse_args()

    java_path = pathlib.Path(args.java)
    rust_path = pathlib.Path(args.rust)
    for p in (java_path, rust_path):
        if not p.exists():
            print(f"error: file not found: {p}", file=sys.stderr)
            return 2

    package, _, class_name = args.java_class.rpartition(".")
    java = parse_java(java_path, package, class_name)
    rust = parse_rust(rust_path)

    if not java:
        print(f"error: no native declarations found in {java_path}", file=sys.stderr)
        return 2

    errors = validate(java, rust)
    checked = len(java)
    symbols: set = set()
    if args.symbols:
        symbols = read_exported_symbols(pathlib.Path(args.symbols))
        if symbols:
            sym_errors = validate_symbols(java, symbols)
            errors += sym_errors
            print(
                f"   symbol check: {checked} declarations vs {len(symbols)} exported Java_ symbols"
            )
        else:
            print("   symbol check skipped (no symbol tool / empty output)")
    if args.report:
        report = build_report(
            java_path, args.java_class, rust_path, java, rust, symbols
        )
        write_report(report, pathlib.Path(args.report))
        print(f"   mapping report: {args.report}")
    if errors:
        print(f"JNI signature check: {checked} declarations, {errors} error(s)")
        return 1
    print(f"✅ JNI signature check: {checked} declarations match {len(rust)} Rust exports")
    return 0


if __name__ == "__main__":
    sys.exit(main())
