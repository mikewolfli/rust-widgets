// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The language families the built-in lexer understands.

use super::DEFAULT_TAB_WIDTH;
/// Language families understood by the built-in lexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum LanguageId {
    /// Rust source (`//`, `/* */`, `"`, `'`, `#[]`).
    Rust,
    /// Python source (`#`, `"""`, `'''`) — indentation-sensitive, no block comments.
    Python,
    /// JavaScript or TypeScript source.
    JavaScript,
    /// Go source, including backtick raw strings.
    Go,
    /// Zig source (`//` line comments, `\\\\` multiline string lines, `"`, `'`).
    Zig,
    /// C-family source, including `#define` preprocessor lines.
    CLike,
    /// Java source.
    Java,
    /// C# source, including `#if` preprocessor lines.
    CSharp,
    /// C++ source.
    Cpp,
    /// Kotlin source.
    Kotlin,
    /// Swift source.
    Swift,
    /// HTML source, including `<!-- … -->` comments.
    Html,
    /// XML source, including `<!-- … -->` comments.
    Xml,
    /// JSON data (no comments; `true`/`false`/`null` are keywords).
    Json,
    /// CSS source (block comments only).
    Css,
    /// YAML source (`#` comments).
    Yaml,
    /// TOML source (`#` comments).
    Toml,
    /// Shell source (`#` comments).
    Shell,
    /// SQL source (`--` line comments, `/* */` block comments).
    Sql,
    /// Markdown-ish plain text.
    Markdown,
    /// No highlighting at all.
    #[default]
    PlainText,
}

impl LanguageId {
    /// Returns the display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::Go => "Go",
            Self::Zig => "Zig",
            Self::CLike => "C",
            Self::Java => "Java",
            Self::CSharp => "C#",
            Self::Cpp => "C++",
            Self::Kotlin => "Kotlin",
            Self::Swift => "Swift",
            Self::Html => "HTML",
            Self::Xml => "XML",
            Self::Json => "JSON",
            Self::Css => "CSS",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
            Self::Shell => "Shell",
            Self::Sql => "SQL",
            Self::Markdown => "Markdown",
            Self::PlainText => "Plain Text",
        }
    }

    /// Returns the number of spaces a tab expands to for this language.
    pub fn default_tab_width(self) -> usize {
        match self {
            // Markup and data formats are conventionally two-space indented.
            Self::Html | Self::Xml | Self::Json | Self::Css => 2,
            _ => DEFAULT_TAB_WIDTH,
        }
    }

    /// Returns `true` when `#` starts a line comment instead of a preprocessor.
    pub fn uses_hash_comments(self) -> bool {
        matches!(self, Self::Python | Self::Shell | Self::Yaml | Self::Toml)
    }

    /// Returns `true` when `/* ... */` block comments are supported.
    pub fn supports_block_comments(self) -> bool {
        matches!(
            self,
            Self::Rust
                | Self::JavaScript
                | Self::Go
                | Self::CLike
                | Self::Java
                | Self::CSharp
                | Self::Cpp
                | Self::Kotlin
                | Self::Swift
                | Self::Css
                | Self::Sql
        )
    }

    /// Returns `true` when `#`-prefixed preprocessor lines are supported.
    pub fn supports_preprocessor(self) -> bool {
        matches!(self, Self::Rust | Self::CLike | Self::Cpp | Self::CSharp)
    }

    /// Returns `true` when `//` starts a line comment.
    pub fn supports_slash_comments(self) -> bool {
        matches!(self, |Self::Rust| Self::JavaScript
            | Self::Go
            | Self::Zig
            | Self::CLike
            | Self::Java
            | Self::CSharp
            | Self::Cpp
            | Self::Kotlin
            | Self::Swift)
    }

    /// Returns `true` when `--` starts a line comment.
    pub fn supports_dash_comments(self) -> bool {
        matches!(self, Self::Sql)
    }

    /// Returns `true` when `<!-- … -->` comments are supported.
    pub fn supports_html_comments(self) -> bool {
        matches!(self, Self::Html | Self::Xml)
    }

    /// Returns `true` when a single-quoted literal is a character, not a string.
    pub fn single_quote_is_char(self) -> bool {
        matches!(
            self,
            Self::Rust
                | Self::Go
                | Self::Zig
                | Self::CLike
                | Self::Java
                | Self::CSharp
                | Self::Cpp
                | Self::Kotlin
                | Self::Swift
        )
    }

    /// Returns `true` when `ch` may appear inside an identifier.
    ///
    /// Also used for word-wise caret movement and whole-word search, so it is
    /// public: plugins can override the notion of "word" for their language.
    pub fn is_word_char(self, ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    /// Returns the keyword set of the language family.
    pub fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &[
                "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
                "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
                "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct", "super",
                "trait", "true", "type", "union", "unsafe", "use", "where", "while",
            ],
            Self::Python => &[
                "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
                "continue", "def", "del", "elif", "else", "except", "finally", "for", "from",
                "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass",
                "raise", "return", "try", "while", "with", "yield",
            ],
            Self::JavaScript => &[
                "async",
                "await",
                "break",
                "case",
                "catch",
                "class",
                "const",
                "continue",
                "default",
                "delete",
                "do",
                "else",
                "export",
                "extends",
                "false",
                "finally",
                "for",
                "function",
                "if",
                "import",
                "in",
                "instanceof",
                "let",
                "new",
                "null",
                "of",
                "return",
                "static",
                "super",
                "switch",
                "this",
                "throw",
                "true",
                "try",
                "typeof",
                "undefined",
                "var",
                "void",
                "while",
                "yield",
            ],
            Self::Go => &[
                "break",
                "case",
                "chan",
                "const",
                "continue",
                "default",
                "defer",
                "else",
                "fallthrough",
                "for",
                "func",
                "go",
                "goto",
                "if",
                "import",
                "interface",
                "map",
                "package",
                "range",
                "return",
                "select",
                "struct",
                "switch",
                "type",
                "var",
            ],
            Self::CLike => &[
                "auto", "break", "case", "const", "continue", "default", "do", "else", "enum",
                "extern", "for", "goto", "if", "inline", "register", "restrict", "return",
                "sizeof", "static", "struct", "switch", "typedef", "union", "volatile", "while",
            ],
            Self::Java => &[
                "abstract",
                "assert",
                "boolean",
                "break",
                "byte",
                "case",
                "catch",
                "char",
                "class",
                "const",
                "continue",
                "default",
                "do",
                "double",
                "else",
                "enum",
                "extends",
                "false",
                "final",
                "finally",
                "float",
                "for",
                "goto",
                "if",
                "implements",
                "import",
                "instanceof",
                "int",
                "interface",
                "long",
                "native",
                "new",
                "null",
                "package",
                "private",
                "protected",
                "public",
                "return",
                "short",
                "static",
                "super",
                "switch",
                "synchronized",
                "this",
                "throw",
                "throws",
                "transient",
                "true",
                "try",
                "void",
                "volatile",
                "while",
            ],
            Self::CSharp => &[
                "abstract",
                "as",
                "base",
                "bool",
                "break",
                "byte",
                "case",
                "catch",
                "char",
                "checked",
                "class",
                "const",
                "continue",
                "decimal",
                "default",
                "delegate",
                "do",
                "double",
                "else",
                "enum",
                "event",
                "explicit",
                "extern",
                "false",
                "finally",
                "fixed",
                "float",
                "for",
                "foreach",
                "goto",
                "if",
                "implicit",
                "in",
                "int",
                "interface",
                "internal",
                "is",
                "lock",
                "long",
                "namespace",
                "new",
                "null",
                "object",
                "operator",
                "out",
                "override",
                "params",
                "private",
                "protected",
                "public",
                "readonly",
                "ref",
                "return",
                "sbyte",
                "sealed",
                "short",
                "sizeof",
                "stackalloc",
                "static",
                "string",
                "struct",
                "switch",
                "this",
                "throw",
                "true",
                "try",
                "typeof",
                "uint",
                "ulong",
                "unchecked",
                "unsafe",
                "ushort",
                "using",
                "virtual",
                "void",
                "volatile",
                "while",
            ],
            Self::Cpp => &[
                "alignas",
                "alignof",
                "auto",
                "bool",
                "break",
                "case",
                "catch",
                "char",
                "class",
                "const",
                "constexpr",
                "continue",
                "decltype",
                "default",
                "delete",
                "do",
                "double",
                "else",
                "enum",
                "explicit",
                "extern",
                "false",
                "float",
                "for",
                "friend",
                "goto",
                "if",
                "inline",
                "int",
                "long",
                "mutable",
                "namespace",
                "new",
                "noexcept",
                "nullptr",
                "operator",
                "private",
                "protected",
                "public",
                "register",
                "return",
                "short",
                "signed",
                "sizeof",
                "static",
                "struct",
                "switch",
                "template",
                "this",
                "throw",
                "true",
                "try",
                "typedef",
                "typename",
                "union",
                "unsigned",
                "using",
                "virtual",
                "void",
                "volatile",
                "while",
            ],
            Self::Kotlin => &[
                "as",
                "break",
                "class",
                "continue",
                "do",
                "else",
                "false",
                "for",
                "fun",
                "if",
                "in",
                "interface",
                "is",
                "null",
                "object",
                "package",
                "return",
                "super",
                "this",
                "throw",
                "true",
                "try",
                "typealias",
                "val",
                "var",
                "when",
                "while",
            ],
            Self::Swift => &[
                "Self",
                "as",
                "associatedtype",
                "break",
                "case",
                "catch",
                "class",
                "continue",
                "default",
                "defer",
                "deinit",
                "do",
                "else",
                "enum",
                "extension",
                "fallthrough",
                "false",
                "fileprivate",
                "for",
                "func",
                "guard",
                "if",
                "import",
                "in",
                "init",
                "inout",
                "internal",
                "is",
                "let",
                "nil",
                "open",
                "operator",
                "private",
                "protocol",
                "public",
                "repeat",
                "rethrows",
                "return",
                "self",
                "static",
                "struct",
                "subscript",
                "switch",
                "throw",
                "throws",
                "true",
                "try",
                "typealias",
                "var",
                "where",
                "while",
            ],
            // Zig keyword set from the language reference. `false`, `null`, `true` and
            // `undefined` are keywords too, because they are reserved words rather than
            // type names. Kept in byte order for the binary-search lookup.
            Self::Zig => &[
                "addrspace",
                "align",
                "allowzero",
                "and",
                "anyframe",
                "anytype",
                "asm",
                "async",
                "await",
                "break",
                "callconv",
                "catch",
                "comptime",
                "const",
                "continue",
                "defer",
                "else",
                "enum",
                "errdefer",
                "error",
                "export",
                "extern",
                "false",
                "fn",
                "for",
                "if",
                "inline",
                "linksection",
                "noalias",
                "noinline",
                "nosuspend",
                "null",
                "opaque",
                "or",
                "orelse",
                "packed",
                "pub",
                "resume",
                "return",
                "struct",
                "suspend",
                "switch",
                "test",
                "threadlocal",
                "true",
                "try",
                "undefined",
                "union",
                "unreachable",
                "usingnamespace",
                "var",
                "volatile",
                "while",
            ],
            Self::Html | Self::Xml | Self::Css => &[],
            Self::Json => &["false", "null", "true"],
            Self::Yaml => &["false", "no", "null", "off", "on", "true", "yes"],
            Self::Toml => &["false", "true"],
            Self::Shell => &[
                "alias", "break", "case", "continue", "declare", "do", "done", "elif", "else",
                "esac", "export", "fi", "for", "function", "if", "in", "local", "readonly",
                "return", "select", "shift", "source", "then", "time", "unset", "until", "while",
            ],
            Self::Sql => &[
                "add",
                "all",
                "alter",
                "and",
                "as",
                "between",
                "by",
                "case",
                "check",
                "column",
                "create",
                "default",
                "delete",
                "distinct",
                "drop",
                "else",
                "end",
                "exists",
                "foreign",
                "from",
                "group",
                "having",
                "in",
                "index",
                "inner",
                "insert",
                "into",
                "is",
                "join",
                "key",
                "left",
                "like",
                "limit",
                "not",
                "null",
                "offset",
                "on",
                "or",
                "order",
                "outer",
                "primary",
                "references",
                "right",
                "select",
                "set",
                "table",
                "then",
                "union",
                "unique",
                "update",
                "values",
                "view",
                "when",
                "where",
            ],
            Self::Markdown | Self::PlainText => &[],
        }
    }

    /// Returns the built-in type and primitive names of the language family.
    pub fn type_names(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &[
                "Arc", "Box", "Option", "Rc", "Result", "String", "Vec", "bool", "char", "f32",
                "f64", "i128", "i16", "i32", "i64", "i8", "isize", "str", "u128", "u16", "u32",
                "u64", "u8", "usize",
            ],
            Self::Python => {
                &["bool", "bytes", "dict", "float", "int", "list", "set", "str", "tuple"]
            }
            Self::JavaScript => {
                &["Array", "Boolean", "Map", "Number", "Object", "Promise", "Set", "String"]
            }
            Self::Go => {
                &["bool", "byte", "error", "float32", "float64", "int", "rune", "string", "uint"]
            }
            Self::CLike => &["char", "double", "float", "int", "long", "short", "void"],
            Self::Java => &[
                "Object", "String", "boolean", "byte", "char", "double", "float", "int", "long",
                "short", "void",
            ],
            Self::CSharp => &[
                "bool", "byte", "char", "decimal", "double", "float", "int", "long", "object",
                "sbyte", "short", "string", "uint", "ulong", "ushort", "void",
            ],
            Self::Cpp => {
                &["bool", "char", "double", "float", "int", "long", "short", "size_t", "void"]
            }
            Self::Kotlin => &[
                "Any", "Boolean", "Byte", "Char", "Double", "Float", "Int", "Long", "Short",
                "String", "Unit",
            ],
            Self::Swift => {
                &["Any", "Bool", "Character", "Double", "Float", "Int", "String", "Void"]
            }
            Self::Sql => &["boolean", "date", "int", "integer", "text", "timestamp", "varchar"],
            // Zig primitive types, including the C ABI aliases. Byte-ordered for the
            // binary-search lookup. `anyframe`/`anytype` are keywords and live in the
            // keyword table instead; `anyerror` is a primitive type and lives here.
            Self::Zig => &[
                "anyerror",
                "bool",
                "c_char",
                "c_int",
                "c_long",
                "c_longdouble",
                "c_longlong",
                "c_short",
                "c_uint",
                "c_ulong",
                "c_ulonglong",
                "c_ushort",
                "comptime_float",
                "comptime_int",
                "f128",
                "f16",
                "f32",
                "f64",
                "f80",
                "i128",
                "i16",
                "i32",
                "i64",
                "i8",
                "isize",
                "noreturn",
                "type",
                "u128",
                "u16",
                "u32",
                "u64",
                "u8",
                "usize",
                "void",
            ],
            Self::Html
            | Self::Xml
            | Self::Json
            | Self::Css
            | Self::Yaml
            | Self::Toml
            | Self::Shell
            | Self::Markdown
            | Self::PlainText => &[],
        }
    }

    /// Returns `true` when `#` opens a line comment for this language.
    pub fn comment_prefix(self) -> &'static str {
        if self.supports_dash_comments() {
            "-- "
        } else if self.uses_hash_comments() {
            "# "
        } else {
            "// "
        }
    }

    /// Returns `true` when the language has a line-comment syntax at all.
    ///
    /// This is the `D2` predicate: a structure-aware command is *meaningless* for
    /// a language that has no such structure, and inserting `// ` into plain text
    /// or a Markdown heading is not a degraded toggle, it is corruption. Callers
    /// consult this and refuse rather than guess, so the failure is visible.
    pub fn supports_line_comments(self) -> bool {
        !matches!(
            self,
            Self::Markdown | Self::PlainText | Self::Json | Self::Html | Self::Xml | Self::Css
        )
    }

    /// Returns `true` when bracket/quote auto-pairing is meaningful.
    ///
    /// Pairing is **not** degraded by language: it is already gated by
    /// `CodeEditorConfig::auto_close_brackets`, which is the user's
    /// explicit choice, and inserting a matching delimiter is a formatting
    /// preference rather than a claim about grammar. The one command whose
    /// incorrect guess corrupts text is [`Self::line_comment_prefix`], which is why
    /// that one *is* degraded. This predicate exists so a host can still ask the
    /// question without the editor deciding for it.
    pub fn supports_auto_pairing(self) -> bool {
        !matches!(self, Self::Markdown | Self::PlainText)
    }

    /// Returns the line-comment prefix, or `None` when there is none.
    ///
    /// The `Option` is what makes the degradation explicit at the call site: a
    /// command that needs a comment token cannot silently fall back to a wrong
    /// one.
    pub fn line_comment_prefix(self) -> Option<&'static str> {
        if self.supports_line_comments() {
            Some(self.comment_prefix())
        } else {
            None
        }
    }
}
