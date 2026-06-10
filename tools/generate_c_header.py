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
    "*mut *mut u8": "uint8_t**",
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


def render_header(functions: list[FunctionDecl]) -> str:
    """Render a sorted C header from parsed function declarations."""
    lines: list[str] = []
    lines.append("#ifndef RW_GENERATED_H")
    lines.append("#define RW_GENERATED_H")
    lines.append("")
    lines.append("#include <stdbool.h>")
    lines.append("#include <stdint.h>")
    lines.append("")
    lines.append("#ifdef __cplusplus")
    lines.append('extern "C" {')
    lines.append("#endif")
    lines.append("")
    lines.append("/* Auto-generated from src/bindings/binding_impl.rs */")
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
    header = render_header(functions)

    output_path.write_text(header, encoding="utf-8")
    print(f"generated {output_path} with {len(functions)} declarations")


if __name__ == "__main__":
    main()
