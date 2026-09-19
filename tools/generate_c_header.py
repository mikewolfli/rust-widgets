#!/usr/bin/env python3
"""Generate C header from Rust extern "C" function declarations."""

from __future__ import annotations

import argparse
import pathlib
import re
from dataclasses import dataclass


@dataclass
class FunctionDecl:
    """A single C function declaration parsed from a Rust extern "C" block."""

    name: str
    params: list[tuple[str, str]]
    return_type: str


TYPE_MAP = {
    "u64": "uint64_t",
    "u8": "uint8_t",
    "i64": "int64_t",
    "c_int": "int",
    "c_uint": "unsigned int",
    "c_float": "float",
    "CBool": "bool",
    "*const c_char": "const char*",
    "*mut c_char": "char*",
    "*const u64": "const uint64_t*",
    "*mut u64": "uint64_t*",
    "*const u8": "const uint8_t*",
    "*mut u8": "uint8_t*",
    "*mut i64": "int64_t*",
    "*mut *mut u8": "uint8_t**",
    "*mut *mut c_char": "char**",
}


def map_type(rust_type: str) -> str:
    """Map a Rust type string to its C equivalent using TYPE_MAP."""
    normalized = " ".join(rust_type.strip().split())
    if normalized in TYPE_MAP:
        return TYPE_MAP[normalized]
    if normalized.startswith("*const "):
        inner = normalized.replace("*const ", "", 1)
        mapped_inner = TYPE_MAP.get(inner, inner)
        return f"const {mapped_inner}*"
    if normalized.startswith("*mut "):
        inner = normalized.replace("*mut ", "", 1)
        mapped_inner = TYPE_MAP.get(inner, inner)
        return f"{mapped_inner}*"
    return TYPE_MAP.get(normalized, normalized)


def parse_bindings(source: str) -> list[FunctionDecl]:
    """Extract all extern "C" function declarations from Rust source."""
    pattern = re.compile(
        r"pub\s+(?:unsafe\s+)?extern \"C\" fn\s+"
        r"(?P<name>[a-zA-Z0-9_]+)\s*\((?P<params>.*?)\)"
        r"\s*(?:->\s*(?P<ret>[^\{]+))?\{",
        re.DOTALL,
    )

    functions: list[FunctionDecl] = []
    for match in pattern.finditer(source):
        name = match.group("name").strip()
        if not name.startswith("rw_"):
            continue

        raw_params = match.group("params").strip()
        params: list[tuple[str, str]] = []
        if raw_params:
            for line in raw_params.split(","):
                token = line.strip()
                if not token:
                    continue
                if ":" not in token:
                    continue
                param_name, rust_type = token.split(":", 1)
                mapped = map_type(rust_type.strip())
                params.append((param_name.strip(), mapped))

        raw_return = (match.group("ret") or "").strip()
        return_type = "void" if raw_return == "" else map_type(raw_return)
        functions.append(
            FunctionDecl(name=name, params=params, return_type=return_type)
        )

    return functions


def parse_value_kinds(source: str) -> list[tuple[str, int]]:
    """Extract the `rw_value_kind` discriminants from the Rust source.

    Why this is parsed rather than hard-coded here: the enum's values are part of
    the ABI contract, and every language binding must use the same numbers. A
    hard-coded copy in this script would be a third place they could drift (the
    Rust constants and each binding are the other two). Parsing keeps
    `binding_impl.rs` the single source of truth, and `check_abi.sh` fails when
    the committed header disagrees with what this produces.

    Before this existed the header published no discriminators at all, so each
    binding declared its own plain integer constants by hand — and all three
    stopped at `RW_VALUE_STRING`, missing the `Color`/`Rect` kinds that were
    appended later. The visible effect was that reading a colour property through
    Python/Node/C++ returned "no such property" *and* leaked the string the ABI
    had allocated, because the caller had no constant to match and never reached
    its free call.
    """
    pattern = re.compile(
        r"^const\s+(RW_VALUE_[A-Z_]+):\s*c_int\s*=\s*(-?\d+);", re.MULTILINE
    )
    return [(match.group(1), int(match.group(2))) for match in pattern.finditer(source)]


def render_header(
    functions: list[FunctionDecl], value_kinds: list[tuple[str, int]]
) -> str:
    """Render a sorted C header from parsed function and enum declarations."""
    lines: list[str] = []
    lines.append("#ifndef RW_GENERATED_H")
    lines.append("#define RW_GENERATED_H")
    lines.append("")
    lines.append("/* Auto-generated from src/bindings/binding_impl.rs. Do not edit. */")
    lines.append("")
    lines.append("#include <stdbool.h>")
    lines.append("#include <stdint.h>")
    lines.append("")
    lines.append("/*")
    lines.append(" * `ObjectId` is the handle type every creation and query function speaks.")
    lines.append(" * It is a typedef rather than a bare `uint64_t` because the cookbook's C")
    lines.append(" * examples use the name, and a header that omits it makes every one of them")
    lines.append(" * fail to compile for a reader who copies them.")
    lines.append(" */")
    lines.append("typedef uint64_t ObjectId;")
    lines.append("")
    lines.append("/*")
    lines.append(" * The `rw_value_kind` discriminants written by `rw_get_widget_property` and")
    lines.append(" * read by `rw_set_widget_property` through their `out_kind`/`kind` argument.")
    lines.append(" *")
    lines.append(" * A binding MUST accept every value here and MUST free `out_str` with")
    lines.append(" * `rw_free_string` whenever it is non-null, including for the non-string")
    lines.append(" * kinds. `RW_VALUE_COLOR` and `RW_VALUE_RECT` carry their payload in")
    lines.append(" * `out_str` (as `#RRGGBBAA` and `x,y,w,h`), not in `out_num`; treating them")
    lines.append(" * as unknown leaks that buffer.")
    lines.append(" */")
    lines.append("typedef enum {")
    for name, value in value_kinds:
        lines.append(f"    {name} = {value},")
    lines.append("} rw_value_kind;")
    lines.append("")
    lines.append("#ifdef __cplusplus")
    lines.append('extern "C" {')
    lines.append("#endif")
    lines.append("")
    for function in sorted(functions, key=lambda item: item.name):
        if function.params:
            params = ", ".join(
                f"{param_type} {param_name}"
                for param_name, param_type in function.params
            )
        else:
            params = "void"
        lines.append(f"{function.return_type} {function.name}({params});")
    lines.append("")
    lines.append("#ifdef __cplusplus")
    lines.append("}")
    lines.append("#endif")
    lines.append("")
    lines.append("#endif /* RW_GENERATED_H */")
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    """Parse bindings and write the generated C header to disk."""
    parser = argparse.ArgumentParser(
        description="Generate C header from Rust C ABI bindings"
    )
    parser.add_argument(
        "--bindings",
        default="src/bindings/binding_impl.rs",
        help="Path to Rust bindings implementation module",
    )
    parser.add_argument(
        "--output",
        default="examples/rust_widgets.generated.h",
        help="Output header path",
    )
    args = parser.parse_args()

    bindings_path = pathlib.Path(args.bindings)
    output_path = pathlib.Path(args.output)

    source = bindings_path.read_text(encoding="utf-8")
    functions = parse_bindings(source)
    value_kinds = parse_value_kinds(source)
    if not value_kinds:
        # A header without the discriminators is exactly the defect this parsing was
        # added to fix, so an empty result is an error rather than an empty enum.
        raise SystemExit(
            f"no RW_VALUE_* constants found in {bindings_path}; the generated header "
            "would publish no value-kind discriminators, and every binding would have "
            "to guess them (which is how RW_VALUE_COLOR/RECT were missed)"
        )
    header = render_header(functions, value_kinds)

    output_path.write_text(header, encoding="utf-8")
    print(
        f"generated {output_path} with {len(functions)} declarations "
        f"and {len(value_kinds)} value kinds"
    )


if __name__ == "__main__":
    main()
