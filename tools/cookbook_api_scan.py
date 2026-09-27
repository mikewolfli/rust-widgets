#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the reverse
# injection that keeps it honest — in tools/check_cookbook.sh, which is the only caller. That file
# is the documentation of record.
#
# The cookbook is the crate's API reference in three languages. A name it presents as an API that
# `src/` does not define is a reader following documentation into a compile error — which happened
# for 13 names, identically in all three editions, before this check existed.
#
# Prints one `finding: ...` line per problem, then a `checked=N failed=M` summary line.
# Exit status is always 0; the shell wrapper makes the pass/fail judgement, so the exit code and the
# printed summary cannot disagree.

import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
COOKBOOKS = ["cookbook/en", "cookbook/zh-CN", "cookbook/zh-TW"]

# A claim is a `Owner::Member` pair, but not every such pair is a claim that the member *exists*.
# Two forms are deliberately excluded, because the cookbook uses them to say the opposite:
#
#   * a **glob** — `Platform::create_*` names a family, not a member. The `*` is part of the
#     name, and a name that is not a member cannot be a missing member.
#   * a **negative mention** — `platform::virtual_keyboard` appears in a sentence saying "no such
#     module exists", which is the most correct thing a reference can say about it.
#
# The second is handled by NEGATIVE_MENTION below rather than by the regex: the *surrounding
# sentence* is what makes it a denial.
API_CLAIM = re.compile(r"`([A-Za-z_][A-Za-z0-9_]*)::([A-Za-z_][A-Za-z0-9_]*)`")

# A sentence that denies the name's existence. A finding inside one of these is the reference
# being *right*, so it is skipped. Matched case-insensitively against the whole line, because the
# denial and the name routinely sit in different clauses (and in all three languages).
NEGATIVE_MENTION = re.compile(
    r"no such (?:module|type|api|item)|does not exist|was removed|was deleted|"
    r"survives only|never compile|could never|不再存在|已移除|已删除|不存在|"
    r"已移除|已被删除|不存在",
    re.IGNORECASE,
)

# Fence languages whose contents are not Rust API claims.
NON_RUST_FENCES = {
    "console", "bash", "sh", "shell", "toml", "ini", "json", "yaml", "yml",
    "c", "cpp", "h", "hpp", "java", "python", "py", "javascript", "js", "typescript",
    "text", "plaintext", "diff", "xml", "html", "css", "sql",
}


def rust_sources():
    """Every `.rs` file's text, concatenated once so lookups are cheap."""
    blobs = []
    for path in (REPO_ROOT / "src").rglob("*.rs"):
        blobs.append(path.read_text(encoding="utf-8", errors="replace"))
    return "\n".join(blobs)


def rust_examples():
    """The examples tree too: a chapter may document a probe the crate ships as an example."""
    blobs = []
    for path in (REPO_ROOT / "examples").rglob("*.rs"):
        blobs.append(path.read_text(encoding="utf-8", errors="replace"))
    return "\n".join(blobs)


def strip_non_rust_fences(text):
    """Remove fenced blocks whose language is not Rust, so their identifiers are not scanned."""
    out = []
    fence = None
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("```"):
            if fence is None:
                lang = stripped[3:].strip().lower()
                fence = lang
                # Keep Rust and language-less fences; drop everything else.
                if lang in NON_RUST_FENCES:
                    out.append("")  # keep the line count, drop the content
                    continue
            else:
                fence = None
            out.append("")
            continue
        if fence is not None and fence in NON_RUST_FENCES:
            out.append("")
        else:
            out.append(line)
    return "\n".join(out)


def main():
    src = rust_sources()
    examples = rust_examples()
    corpus = src + "\n" + examples

    checked = 0
    findings = []

    # A `Owner::Member` claim can be four different things, and only two of them are checkable
    # without inventing problems. Deciding which, from the corpus:
    #
    #   * **enum / union variant** — `WidgetKind::Button`. Validated by "is the variant
    #     declared inside that enum's body", not by looking for a `fn Button`.
    #   * **module path** — `platform::ime_linux`. Validated by "does that directory or file
    #     exist", not by looking for a type called `platform`.
    #   * **associated function / constant** — `Rect::new`. Validated by a real definition.
    #
    # An earlier revision looked for `fn Member` for every shape, which reported 209 findings
    # almost all of them enum variants spelled `Owner::Variant`. A gate that reports the
    # ordinary way of naming a variant is a gate nobody reads — the same failure mode as one
    # that never fires.
    enum_bodies = {}
    for match in re.finditer(
        r"\b(?:pub(?:\([^)]*\))?\s+)?enum\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)[^{]*\{",
        corpus,
    ):
        # Walk from the opening brace to the one that closes it. The scan **starts at the brace
        # itself** (`match.end() - 1`) and counts it, so the first `}` that brings the count back
        # to zero is the end of the enum body. An earlier revision initialised `depth` to 0 and
        # entered the loop before seeing the brace, which made the body span the *next* enum as
        # well — so `WidgetKind` looked like it declared no variants when it declares 180.
        depth = 0
        start = match.end() - 1
        i = start
        while i < len(corpus):
            char = corpus[i]
            if char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        enum_bodies.setdefault(match.group("name"), corpus[start : i + 1])

    module_dirs = set()
    for path in (REPO_ROOT / "src").rglob("*"):
        if path.is_dir():
            module_dirs.add(path.name)
        elif path.suffix == ".rs":
            module_dirs.add(path.stem)

    for book in COOKBOOKS:
        base = REPO_ROOT / book / "src"
        if not base.exists():
            findings.append(f"{book}: no `src/` directory")
            continue
        for path in sorted(base.rglob("*.md")):
            text = strip_non_rust_fences(path.read_text(encoding="utf-8", errors="replace"))
            rel = path.relative_to(REPO_ROOT).as_posix()
            for line_no, line in enumerate(text.splitlines(), 1):
                # A line that denies existence is the reference being correct, not wrong.
                if NEGATIVE_MENTION.search(line):
                    continue
                for owner, member in API_CLAIM.findall(line):
                    # A claim is about *the crate* only when the owner is one of its types.
                    # `std::` and `core::` are not ours, so they are skipped rather than excused.
                    if owner in {"std", "core", "alloc", "self", "crate", "super"}:
                        continue
                    checked += 1

                    # A **module path** (`app::App`, `platform::ime_linux`, `runtime::register`) is
                    # valid when that module exists — the `Owner` is a module name, not a type, so
                    # looking for a type called `app` would always fail. Checked before the variant
                    # case because a module and a type can share a name in different scopes.
                    if owner in module_dirs and owner not in enum_bodies:
                        if member in module_dirs or re.search(
                            rf"\b(?:struct|enum|trait|type|union|fn|mod)\s+{member}\b", corpus
                        ):
                            continue
                        findings.append(
                            f"{rel}:{line_no}: `{owner}::{member}` — no `{member}` module, file or "
                            f"item under `{owner}`"
                        )
                        continue

                    # An **associated item** — a method, constant, type or field — is valid on its
                    # own evidence, and it is checked **before** the variant case. A type is often
                    # both (`IconName` is an enum *and* has `IconName::data`; `Event` is an enum
                    # *and* has `Event::is_touch`), and `Owner::Member` does not say which the
                    # chapter meant. Checking the definition first means a real method is never
                    # reported as a missing variant — which is what an earlier revision did for
                    # every method on an enum, and those were most of its remaining findings.
                    if re.search(rf"\bfn\s+{member}\b", corpus):
                        continue
                    if re.search(rf"\b(?:const|type|struct|enum|trait)\s+{member}\b", corpus):
                        continue
                    if re.search(rf"\b{member}\s*:", corpus):
                        continue

                    # A **variant** (`WidgetKind::Button`): valid when the enum declares it.
                    body = enum_bodies.get(owner)
                    if body is not None:
                        if re.search(rf"\b{member}\b", body):
                            continue
                        findings.append(
                            f"{rel}:{line_no}: `{owner}::{member}` — `{owner}` declares no variant "
                            f"named `{member}`, and no such method, constant, type or field exists"
                        )
                        continue

                    findings.append(
                        f"{rel}:{line_no}: `{owner}::{member}` — `{member}` is not defined as a "
                        f"method, constant, type, field or variant anywhere in src/ or examples/"
                    )

    for finding in findings:
        print(f"finding: {finding}")
    print(f"checked={checked} failed={len(findings)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
