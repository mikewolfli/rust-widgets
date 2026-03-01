#!/usr/bin/env python3
"""Generate C header declarations from src/bindings/mod.rs extern "C" functions."""

from __future__ import annotations

import argparse
import pathlib
import re
from dataclasses import dataclass


@dataclass
class FunctionDecl:
    name: str
    params: list[tuple[str, str]]
    return_type: str


TYPE_MAP = {
    "u64": "uint64_t",
    "c_int": "int",
    "c_uint": "unsigned int",
    "c_float": "float",
    "CBool": "bool",
    "*const c_char": "const char*",
    "*mut c_char": "char*",
    "*const u64": "const uint64_t*",
    "*mut u64": "uint64_t*",
}


def map_type(rust_type: str) -> str:
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
    pattern = re.compile(
        r"pub extern \"C\" fn\s+(?P<name>[a-zA-Z0-9_]+)\s*\((?P<params>.*?)\)\s*(?:->\s*(?P<ret>[^\{]+))?\{",
        re.DOTALL,
    )

    functions: list[FunctionDecl] = []
    for match in pattern.finditer(source):
        name = match.group("name").strip()
        if not name.startswith("rust_widgets_"):
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
                params.append((param_name.strip(), map_type(rust_type.strip())))

        raw_return = (match.group("ret") or "").strip()
        return_type = "void" if raw_return == "" else map_type(raw_return)
        functions.append(FunctionDecl(name=name, params=params, return_type=return_type))

    return functions


def render_header(functions: list[FunctionDecl]) -> str:
    lines: list[str] = []
    lines.append("#ifndef RUST_WIDGETS_GENERATED_H")
    lines.append("#define RUST_WIDGETS_GENERATED_H")
    lines.append("")
    lines.append("#include <stdbool.h>")
    lines.append("#include <stdint.h>")
    lines.append("")
    lines.append("#ifdef __cplusplus")
    lines.append('extern "C" {')
    lines.append("#endif")
    lines.append("")
    lines.append("/* Auto-generated from src/bindings/mod.rs */")
    for function in sorted(functions, key=lambda item: item.name):
        if function.params:
            params = ", ".join(f"{param_type} {param_name}" for param_name, param_type in function.params)
        else:
            params = "void"
        lines.append(f"{function.return_type} {function.name}({params});")
    lines.append("")
    lines.append("#ifdef __cplusplus")
    lines.append("}")
    lines.append("#endif")
    lines.append("")
    lines.append("#endif /* RUST_WIDGETS_GENERATED_H */")
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate C header from Rust C ABI bindings")
    parser.add_argument(
        "--bindings",
        default="src/bindings/mod.rs",
        help="Path to Rust bindings module",
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
