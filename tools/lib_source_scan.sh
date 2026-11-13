#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Shared Rust source-scanning primitives for the QA gates.
#
# # Why this file exists
#
# Two operations were copy-pasted into most source gates, and the copies had
# drifted apart in ways that changed what the gates measured (W6-36):
#
#   * Comment stripping. The common spelling was `sed -e 's://.*::'`, which also
#     deletes `//` inside a string literal, cannot see a raw string, and was
#     sometimes extended with `-e '/^[[:space:]]*\/\//d'` — deleting whole lines
#     and therefore shifting every line number the gate reported.
#   * Function-body slicing. The common spelling was `awk` from a signature to
#     the first line that looked like a closing brace. For a method (closing
#     brace indented by four spaces) the `^}$` pattern never matched, so the
#     slice ran to end of file and swallowed every following function: a redraw
#     in one function was "announced" by a sibling (W6-15). The `    }` spelling
#     has the opposite defect — the first nested block that closes at the
#     function's own indent ends the slice early.
#
# Both operations are now defined **once**, here, with the semantics the gates
# actually need:
#
#   * Comment stripping removes comment text only. Every line stays on its own
#     line, so a line number computed after stripping is the line number in the
#     file. `//` inside `"…"`, `r#"…"#` and `'…'` is *text*, not a comment.
#   * Body slicing counts braces, and counts only braces that are code — braces
#     inside string/char/raw literals and line comments are ignored. The slice
#     starts at the given line and ends at the matching close brace, so it is
#     the function's own body rather than a fixed window.
#
# # Usage
#
#   . "$ROOT_DIR/tools/lib_source_scan.sh"
#   rw_strip_line_comments src/file.rs           # to stdout
#   grep -rn 'pat' src/ | rw_strip_line_comments # from stdin
#   rw_slice_function_body src/file.rs 1201
#   rw_enclosing_function_line src/file.rs 1250
#   rw_function_line src/file.rs drive_frame
#
# Every helper exits 0 on success. A missing file is a usage error (exit 2), not
# a silent empty result: a gate that strips nothing and then finds nothing must
# not look like a pass.
#
# # What this does NOT understand
#
#   * Block comments (`/* … */`). That matches the `sed -e 's://.*::'` these
#     helpers replace: a block comment is left in place. No gate that matters
#     here relies on block-comment stripping, and pretending to handle nesting
#     would be a second lexer nobody reviews.
#   * Rust's full grammar. A macro body is still just characters: braces inside
#     `macro_rules!` patterns are counted as braces, which is why the slice is
#     used per function rather than per file.
#
# SPDX header above, like every other tools/ file (`tools/add_spdx_headers.py` writes them).

# --------------------------------------------------------------------------
# The lexer, shared by all helpers.
#
# It is one awk program with a `mode` parameter rather than three copies of the
# same character walk:
#
#   mode=strip      print every line with its line comment removed
#   mode=slice      print lines `start`.. and stop after the brace-balanced body
#   mode=enclosing  at line `target`, print the innermost enclosing function
#
# `want=name` / `want=line` selects which fact mode=enclosing prints.
#
# The single quote cannot appear inside a single-quoted shell string, so awk
# derives it with `sprintf("%c", 39)`.
# --------------------------------------------------------------------------
_RW_SOURCE_SCAN_AWK='
BEGIN {
    SQ = sprintf("%c", 39)
    depth = 0
    started = 0
    bail = 0
    pending = ""
    pending_line = 0
    in_raw = 0
    raw_h = 0
}

function name_from_code(code,   rest) {
    rest = code
    sub(/^.*fn[ \t]+/, "", rest)
    sub(/[^A-Za-z0-9_].*/, "", rest)
    return rest
}

function looks_like_fn(code,   probe) {
    probe = code
    sub(/^[ \t]+/, "", probe)
    return probe ~ /^(pub(\([^)]*\))?[ \t]+)?(async[ \t]+)?(const[ \t]+)?(unsafe[ \t]+)?(extern[ \t]+"[^"]*"[ \t]+)?fn[ \t]+[A-Za-z_][A-Za-z0-9_]*/
}

{
    if (mode == "slice" && NR < start) next

    if (mode == "enclosing" && NR == target) {
        if (want == "line") {
            print ((depth in fstart) ? fstart[depth] : 0)
        } else {
            print ((depth in names) ? names[depth] : "")
        }
        exit
    }

    line = $0
    n = length(line)
    out = ""
    i = 1
    in_str = 0
    in_char = 0

    while (i <= n) {
        c = substr(line, i, 1)

        # ---- inside a raw string (may span lines; state is global) --------
        if (in_raw) {
            if (c == "\"") {
                j = i + 1
                h = 0
                while (j <= n && substr(line, j, 1) == "#") { h++; j++ }
                if (h >= raw_h) {
                    out = out substr(line, i, j - i)
                    i = j
                    in_raw = 0
                    continue
                }
            }
            out = out c
            i++
            continue
        }

        # ---- inside an ordinary string -----------------------------------
        if (in_str) {
            out = out c
            if (c == "\\") {
                if (i < n) out = out substr(line, i + 1, 1)
                i += 2
                continue
            }
            if (c == "\"") in_str = 0
            i++
            continue
        }

        # ---- inside a char literal ---------------------------------------
        if (in_char) {
            out = out c
            if (c == "\\") {
                if (i < n) out = out substr(line, i + 1, 1)
                i += 2
                continue
            }
            if (c == SQ) in_char = 0
            i++
            continue
        }

        # ---- code ---------------------------------------------------------
        if (c == "/" && substr(line, i + 1, 1) == "/") break   # line comment

        if (c == "\"") { in_str = 1; out = out c; i++; continue }

        if (c == SQ) {
            # A backslash-quoted character is a char literal, and so is one
            # character between quotes. A lone quote is a lifetime
            # (`&a str` in Rust), which must not start a char literal or the
            # rest of the line would be treated as literal text.
            nxt = substr(line, i + 1, 1)
            if (nxt == "\\" || substr(line, i + 2, 1) == SQ) in_char = 1
            out = out c
            i++
            continue
        }

        if (c == "r") {
            # A raw string opener: r"…", r#"…"#, br#"…"#. The `r` must be at a
            # word boundary, or `error`/`for` would be read as openers.
            prev = (i > 1) ? substr(line, i - 1, 1) : ""
            boundary = (prev !~ /[A-Za-z0-9_]/)
            if (!boundary && prev == "b") {
                before = (i > 2) ? substr(line, i - 2, 1) : ""
                boundary = (before !~ /[A-Za-z0-9_]/)
            }
            if (boundary) {
                j = i + 1
                h = 0
                while (j <= n && substr(line, j, 1) == "#") { h++; j++ }
                if (j <= n && substr(line, j, 1) == "\"") {
                    in_raw = 1
                    raw_h = h
                    out = out substr(line, i, j - i + 1)
                    i = j + 1
                    continue
                }
            }
            out = out c
            i++
            continue
        }

        if (c == "{") {
            depth++
            if (mode == "enclosing") {
                if (pending != "") {
                    names[depth] = pending
                    fstart[depth] = pending_line
                    pending = ""
                } else if (looks_like_fn(out)) {
                    names[depth] = name_from_code(out)
                    fstart[depth] = NR
                } else if ((depth - 1) in names) {
                    names[depth] = names[depth - 1]
                    fstart[depth] = fstart[depth - 1]
                }
            }
            if (mode == "slice") started = 1
            out = out c
            i++
            continue
        }

        if (c == "}") {
            if (mode == "enclosing") {
                delete names[depth]
                delete fstart[depth]
            }
            depth--
            if (mode == "slice" && !started && depth < 0) depth = 0
            out = out c
            i++
            continue
        }

        # A trait declaration has no body: `fn f(&self) -> bool;`. Bailing on
        # the first `;` before any `{` keeps the slice empty instead of running
        # to the end of the file.
        if (mode == "slice" && !started && c == ";") { bail = 1; break }

        out = out c
        i++
    }

    if (bail) exit

    if (mode == "strip") { print out; next }

    if (mode == "slice") {
        print out
        if (started && depth <= 0) exit
        next
    }

    # mode == "enclosing": remember a signature whose `{` is on a later line.
    if (mode == "enclosing" && looks_like_fn(out)) {
        pending = name_from_code(out)
        pending_line = NR
        # itself has no body; do not let it capture the brace of the item that
        # follows.
        if (out ~ /;[ \t]*$/) pending = ""
    }
}
'

# Prints `file` (or stdin when the argument is `-` or absent) with every `//`
# line comment removed. Line count and line order are preserved exactly; only
# the comment text disappears.
rw_strip_line_comments() {
  local file="${1:--}"
  if [ "$file" = "-" ] || [ -z "$file" ]; then
    awk -v mode=strip "$_RW_SOURCE_SCAN_AWK"
    return $?
  fi
  if [ ! -f "$file" ]; then
    printf 'rw_strip_line_comments: no such file: %s\n' "$file" >&2
    return 2
  fi
  awk -v mode=strip "$_RW_SOURCE_SCAN_AWK" "$file"
}

# Prints the body of the function that begins at (or contains) `start_line`.
#
# The slice ends at the function's own closing brace, found by counting braces
# in code only. Output line 1 is `start_line` in the file, so a caller that
# greps `-n` over the result can add `start_line - 1` to recover file line
# numbers.
#
# An unbalanced or bodyless declaration prints nothing and still exits 0: the
# callers' emptiness check is the honest signal, and a hard exit here would
# abort a gate under `set -e` before it could report which function it lost.
rw_slice_function_body() {
  local file="${1:-}" start="${2:-}"
  case "$start" in
    '' | *[!0-9]*)
      printf 'rw_slice_function_body: usage: rw_slice_function_body <file> <start_line>\n' >&2
      return 2
      ;;
  esac
  if [ "$file" = "" ] || [ ! -f "$file" ]; then
    printf 'rw_slice_function_body: no such file: %s\n' "$file" >&2
    return 2
  fi
  awk -v mode=slice -v start="$start" "$_RW_SOURCE_SCAN_AWK" "$file"
}

# Prints the name of the innermost function whose body contains `line`, or an
# empty line when the line is outside every function. The search is structural
# (brace depth plus an anchored `fn` declaration), not "the nearest preceding
# line that mentions `fn`", so a doc comment or a `fn(…)` pointer type cannot
# name the wrong function.
rw_enclosing_function_name() {
  local file="${1:-}" target="${2:-}"
  case "$target" in
    '' | *[!0-9]*)
      printf 'rw_enclosing_function_name: usage: rw_enclosing_function_name <file> <line>\n' >&2
      return 2
      ;;
  esac
  if [ "$file" = "" ] || [ ! -f "$file" ]; then
    printf 'rw_enclosing_function_name: no such file: %s\n' "$file" >&2
    return 2
  fi
  awk -v mode=enclosing -v target="$target" -v want=name "$_RW_SOURCE_SCAN_AWK" "$file"
}

# Prints the declaration line of the innermost function containing `line`, or 0.
rw_enclosing_function_line() {
  local file="${1:-}" target="${2:-}"
  case "$target" in
    '' | *[!0-9]*)
      printf 'rw_enclosing_function_line: usage: rw_enclosing_function_line <file> <line>\n' >&2
      return 2
      ;;
  esac
  if [ "$file" = "" ] || [ ! -f "$file" ]; then
    printf 'rw_enclosing_function_line: no such file: %s\n' "$file" >&2
    return 2
  fi
  awk -v mode=enclosing -v target="$target" -v want=line "$_RW_SOURCE_SCAN_AWK" "$file"
}

# Prints the line number of the first definition of `name`, matching the
# declaration shape at the start of a code line rather than any mention of the
# name. Prints nothing and returns 1 when there is no such definition.
rw_function_line() {
  local file="${1:-}" name="${2:-}"
  if [ "$file" = "" ] || [ ! -f "$file" ] || [ "$name" = "" ]; then
    printf 'rw_function_line: usage: rw_function_line <file> <name>\n' >&2
    return 2
  fi
  local hits
  hits="$(rw_strip_line_comments "$file" \
    | grep -nE "^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?(async[[:space:]]+)?(const[[:space:]]+)?(unsafe[[:space:]]+)?(extern[[:space:]]+\"[^\"]*\"[[:space:]]+)?fn[[:space:]]+${name}[[:space:]]*(\(|<)" \
    || true)"
  if [ -z "$hits" ]; then
    return 1
  fi
  # `sed` reads all of the input (unlike `head`), so a short read cannot make this
  # pipeline fail under `pipefail` when the caller only wants the first match.
  printf '%s\n' "$hits" | sed -n '1s/:.*$//p'
}
