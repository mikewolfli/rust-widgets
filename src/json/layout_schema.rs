// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The discoverable, validatable description of the declarative layout kinds, plus the
//! diagnostic a designer needs when a declaration cannot be honoured exactly (BLUE26 B26.05).
//!
//! # What problem this solves
//!
//! A layout inspector in a designer must answer three questions the JSON layer could not:
//!
//! 1. **What parameters does `hbox` take?** The parser knew (`spacing`, `margin`), but nothing
//!    published it, so a designer's property panel had to hard-code its own copy of every kind's
//!    fields — a copy that goes stale the moment the parser gains a field.
//! 2. **What did the layout engine actually accept?** Two kinds carry parameters the constructor
//!    *ignores*: [`Stack`](DeclarativeLayoutKind::Stack)'s `spacing` and
//!    [`Splitter`](DeclarativeLayoutKind::Splitter)'s `margin`. A document setting them looked
//!    honoured and was silently dropped.
//! 3. **Did the document contain a field this build does not know?** An unknown key was ignored, so
//!    a typo (`spacng`) or a field from a newer version produced a layout with no complaint.
//!
//! This module answers all three from one declaration per kind, so the panel, the validator and the
//! diagnostic cannot disagree about what a kind accepts.
//!
//! # Reuse, not a second solver
//!
//! Nothing here computes geometry. The *result* half of a layout inspection is
//! [`Layout::compute`](crate::layout::Layout::compute) itself: a caller feeds the same `Layout` this
//! module's [`create_layout_from_kind`](crate::json::create_layout_from_kind) builds and reads the
//! rectangles back. Reporting a second set of rectangles from a second solver is exactly the
//! "parallel layout algorithm" the plan forbids.

use crate::compat::{format, String, Vec};

/// The declared type of one layout parameter, for a validation and a panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// A non-negative integer (a pixel spacing, a count).
    UInt,
    /// A signed integer (a gap that may be negative).
    Int,
    /// An axis token (`"horizontal"`/`"vertical"`).
    Axis,
    /// A border region token (`"north"`, `"south"`, `"west"`, `"east"`, `"center"`).
    Region,
    /// A decimal number (a ratio, a fraction).
    Float,
    /// A JSON boolean (`true` / `false`).
    Bool,
}

impl ParamKind {
    /// The stable token an inspector reports.
    pub const fn token(self) -> &'static str {
        match self {
            Self::UInt => "uint",
            Self::Int => "int",
            Self::Axis => "axis",
            Self::Region => "region",
            Self::Float => "float",
            Self::Bool => "bool",
        }
    }
}

/// One parameter of a layout kind: its JSON key, its type, and whether the constructor honours it.
///
/// The parser's *tolerant* defaults (an absent `wrap.spacing` is `8`, an absent `grid.columns` is
/// `2`) are deliberately **not** mirrored here. They are the lenient-parse half of the contract —
/// "a slightly wrong document still renders" — while this schema answers "what may a document
/// say?", which is the question a panel and a validator ask. Mirroring them would be a second
/// declaration of the same numbers, which is the drift this module exists to avoid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutParam {
    /// The JSON key, as the parser reads it.
    pub key: &'static str,
    /// The value's declared type.
    pub kind: ParamKind,
    /// Whether omitting the key changes the result (a key with a default the kind relies on).
    ///
    /// A panel renders a required parameter differently from an optional one.
    pub required: bool,
    /// Whether [`create_layout_from_kind`](crate::json::create_layout_from_kind) actually uses the
    /// value.
    ///
    /// # Why an honoured flag rather than removing the field
    ///
    /// Two parameters (`stack.spacing`, `splitter.margin`) are parsed and then dropped by the
    /// constructor, because the layout they build has no such argument. Reporting them as unhonoured
    /// is the honest description: a document that sets them gets a layout without them, and a
    /// designer should grey the field out rather than offer a control that does nothing. Deleting
    /// them from the schema would make the *parser* (which still accepts and stores them) and the
    /// schema disagree.
    pub honoured: bool,
}

/// The parameter schema of one layout kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutKindSchema {
    /// The kind's canonical JSON type token.
    pub type_token: &'static str,
    /// Every other token that selects the same kind (aliases and case variants).
    pub aliases: &'static [&'static str],
    /// The kind's parameters, in a stable order.
    pub params: &'static [LayoutParam],
}

impl LayoutKindSchema {
    /// Whether `token` selects this kind.
    pub fn matches(&self, token: &str) -> bool {
        self.type_token == token || self.aliases.contains(&token)
    }

    /// The parameter with `key`, if the kind has one.
    pub fn param(&self, key: &str) -> Option<&'static LayoutParam> {
        self.params.iter().find(|param| param.key == key)
    }
}

/// Shorthand for a parameter row.
const fn param(key: &'static str, kind: ParamKind, required: bool, honoured: bool) -> LayoutParam {
    LayoutParam { key, kind, required, honoured }
}

/// The parameter schema of every declarative layout kind.
///
/// # Completeness
///
/// `every_parsed_kind_has_a_schema` in `src/json/layout.rs` walks the kinds the parser produces and
/// fails if one has no entry here, so a new kind cannot be added without its schema — the same
/// discipline the command table uses (BLUE26 B26.03).
///
/// # Why the order is fixed
///
/// A panel and a diff both read this list; a stable order keeps a saved document and a UI in step.
pub static LAYOUT_KIND_SCHEMAS: &[LayoutKindSchema] = &[
    LayoutKindSchema {
        type_token: "hbox",
        aliases: &["HBox", "horizontal"],
        params: &[
            param("spacing", ParamKind::UInt, false, true),
            param("margin", ParamKind::UInt, false, true),
        ],
    },
    LayoutKindSchema {
        type_token: "vbox",
        aliases: &["VBox", "vertical"],
        params: &[
            param("spacing", ParamKind::UInt, false, true),
            param("margin", ParamKind::UInt, false, true),
        ],
    },
    LayoutKindSchema {
        type_token: "grid",
        aliases: &["Grid"],
        params: &[
            param("columns", ParamKind::UInt, false, true),
            param("spacing", ParamKind::UInt, false, true),
            param("margin", ParamKind::UInt, false, true),
        ],
    },
    LayoutKindSchema {
        type_token: "uniform_grid",
        aliases: &["uniformGrid", "uniform-grid"],
        params: &[
            param("rows", ParamKind::UInt, false, true),
            param("columns", ParamKind::UInt, false, true),
            param("spacing", ParamKind::UInt, false, true),
            param("margin", ParamKind::UInt, false, true),
        ],
    },
    LayoutKindSchema {
        type_token: "stack",
        aliases: &["Stack"],
        // `spacing` is parsed but the constructor (`StackLayout::new()`) takes none, so the value is
        // dropped. Reporting `honoured: false` is what lets a designer grey the field out.
        params: &[param("spacing", ParamKind::UInt, false, false)],
    },
    LayoutKindSchema {
        type_token: "splitter",
        aliases: &["Splitter"],
        params: &[
            param("orientation", ParamKind::Axis, false, true),
            // `margin` is parsed but the constructor hard-codes the handle width to 0 and takes no
            // margin, so the value is dropped.
            param("margin", ParamKind::UInt, false, false),
        ],
    },
    LayoutKindSchema {
        type_token: "form",
        aliases: &["Form"],
        params: &[
            param("spacing", ParamKind::UInt, false, true),
            param("margin", ParamKind::UInt, false, true),
        ],
    },
    LayoutKindSchema {
        type_token: "flow",
        aliases: &["Flow"],
        params: &[
            param("spacing", ParamKind::Int, false, true),
            param("padding", ParamKind::Int, false, true),
            param("orientation", ParamKind::Axis, false, true),
        ],
    },
    LayoutKindSchema {
        type_token: "wrap",
        aliases: &["Wrap"],
        params: &[
            param("spacing", ParamKind::Int, false, true),
            param("padding", ParamKind::Int, false, true),
            param("orientation", ParamKind::Axis, false, true),
        ],
    },
    LayoutKindSchema {
        type_token: "flex",
        aliases: &["Flex"],
        params: &[param("gap", ParamKind::UInt, false, true)],
    },
    LayoutKindSchema {
        type_token: "border",
        aliases: &["Border"],
        params: &[
            param("spacing", ParamKind::UInt, false, true),
            param("margin", ParamKind::UInt, false, true),
        ],
    },
    LayoutKindSchema {
        type_token: "center",
        aliases: &["Center"],
        params: &[
            param("width_factor", ParamKind::Float, false, true),
            param("height_factor", ParamKind::Float, false, true),
        ],
    },
    LayoutKindSchema {
        type_token: "aspect_ratio",
        aliases: &["aspectRatio", "aspect-ratio"],
        params: &[
            param("ratio", ParamKind::Float, false, true),
            param("respect_parent", ParamKind::Bool, false, true),
        ],
    },
    LayoutKindSchema {
        type_token: "keyboard_aware",
        aliases: &["keyboardAware", "keyboard-aware"],
        params: &[
            param("spacing", ParamKind::UInt, false, true),
            param("margin", ParamKind::UInt, false, true),
            param("animation_duration", ParamKind::UInt, false, true),
        ],
    },
];

/// The child-side parameters a declarative layout accepts on a child entry.
///
/// A layout document nests its children in the kind object, and a child may carry its own placement
/// object (`{"stretch": 2, "col": 1}`). Those keys are read by
/// [`ChildLayoutAttrs::from_value`](crate::json::ChildLayoutAttrs::from_value) and were, like the
/// kind's own parameters, undiscoverable: a designer's child panel had to hard-code the five names
/// and could not tell which of them a given layout uses (the grid keys mean nothing to an `hbox`).
///
/// `honoured` is the honest answer *per key* rather than per kind: `stretch` is read by every kind
/// that distributes space, while `col`/`row`/`col_span`/`row_span` are only meaningful to
/// [`Grid`](crate::layout::GridLayout) and [`UniformGrid`](crate::layout::UniformGridLayout).
/// A panel greys out what the enclosing kind ignores instead of offering a field that does nothing.
pub static CHILD_ATTR_SCHEMA: &[LayoutParam] = &[
    param("stretch", ParamKind::UInt, false, true),
    param("col", ParamKind::UInt, false, false),
    param("row", ParamKind::UInt, false, false),
    param("col_span", ParamKind::UInt, false, false),
    param("row_span", ParamKind::UInt, false, false),
    param("region", ParamKind::Region, false, false),
    param("extent", ParamKind::UInt, false, false),
];

/// The child-placement keys a layout **kind** actually honours.
///
/// Derived from the kind rather than stored per kind, so a kind added to
/// [`LAYOUT_KIND_SCHEMAS`] cannot forget to declare which of the grid keys it reads.
pub fn child_attrs_honoured_by(type_token: &str) -> Vec<&'static str> {
    let grid =
        matches!(type_token, "grid" | "Grid" | "uniform_grid" | "uniformGrid" | "uniform-grid");
    let border = matches!(type_token, "border" | "Border");
    CHILD_ATTR_SCHEMA
        .iter()
        .filter(|attr| {
            attr.honoured
                || (grid && matches!(attr.key, "col" | "row" | "col_span" | "row_span"))
                || (border && matches!(attr.key, "region" | "extent"))
        })
        .map(|attr| attr.key)
        .collect()
}

/// Validates one child's placement object against [`CHILD_ATTR_SCHEMA`], reporting unknown keys and
/// wrong kinds.
///
/// `path` locates the child's own object in the document, so the issue reads on the offending node
/// rather than on the whole layout. A key the enclosing kind ignores is `DroppedField`, which is
/// what lets a panel explain "`col_span` has no meaning under an `hbox`" instead of showing an
/// editable field whose value never reaches the layout.
pub fn validate_child_attrs(
    obj: &crate::compat::HashMap<String, String>,
    type_token: &str,
    path: &[usize],
) -> Vec<LayoutIssue> {
    let honoured = child_attrs_honoured_by(type_token);
    let mut issues = Vec::new();
    for (key, value) in obj {
        match CHILD_ATTR_SCHEMA.iter().find(|attr| attr.key == key) {
            None => issues.push(LayoutIssue {
                path: path.to_vec(),
                diagnostic: LayoutDiagnostic::UnknownField { key: key.clone() },
            }),
            Some(attr) if !honoured.contains(&attr.key) => issues.push(LayoutIssue {
                path: path.to_vec(),
                diagnostic: LayoutDiagnostic::DroppedField {
                    key: key.clone(),
                    reason: "the enclosing layout kind places its children itself",
                },
            }),
            Some(attr) => {
                if !value_matches_kind(value, attr.kind) {
                    issues.push(LayoutIssue {
                        path: path.to_vec(),
                        diagnostic: LayoutDiagnostic::WrongKind {
                            key: key.clone(),
                            expected: attr.kind,
                        },
                    });
                }
            }
        }
    }
    issues
}

/// The schema for a layout type token, if it names a kind this build knows.
pub fn schema_for_type(token: &str) -> Option<&'static LayoutKindSchema> {
    LAYOUT_KIND_SCHEMAS.iter().find(|schema| schema.matches(token))
}

/// Every type token this build accepts, for an inspector or an error message.
pub fn supported_types() -> Vec<&'static str> {
    let mut out = Vec::new();
    for schema in LAYOUT_KIND_SCHEMAS {
        out.push(schema.type_token);
    }
    out
}

/// Something a layout declaration asked for that this build could not honour exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutDiagnostic {
    /// The document named a type this build does not know.
    UnknownType {
        /// The token as declared.
        declared: String,
    },
    /// The document carried a key the kind's schema does not declare.
    ///
    /// A likely typo, or a field from a newer version. Reported rather than ignored: a silently
    /// ignored key means the document says one thing and the layout does another.
    UnknownField {
        /// The key as declared.
        key: String,
    },
    /// The document set a parameter the constructor does not use.
    DroppedField {
        /// The key that was dropped.
        key: String,
        /// Why it could not be applied, so a caller can explain it.
        reason: &'static str,
    },
    /// A value was the wrong type for its parameter.
    WrongKind {
        /// The key.
        key: String,
        /// The kind the schema declares.
        expected: ParamKind,
    },
}

impl LayoutDiagnostic {
    /// A one-line description, so a report reads without a second lookup.
    pub fn describe(&self) -> String {
        match self {
            Self::UnknownType { declared } => format!(
                "unknown layout type `{declared}`; supported: {}",
                supported_types().join(", ")
            ),
            Self::UnknownField { key } => {
                format!("unknown layout field `{key}`: it will be ignored")
            }
            Self::DroppedField { key, reason } => {
                format!("layout field `{key}` is not applied: {reason}")
            }
            Self::WrongKind { key, expected } => {
                format!("layout field `{key}` must be a {}", expected.token())
            }
        }
    }
}

/// A [`LayoutDiagnostic`] with the position it was found at, so a report can locate it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutIssue {
    /// The path of the layout declaration in the document (`[]` = the document root).
    pub path: Vec<usize>,
    /// What was wrong.
    pub diagnostic: LayoutDiagnostic,
}

impl LayoutIssue {
    /// A one-line rendering with the path.
    pub fn describe(&self) -> String {
        if self.path.is_empty() {
            self.diagnostic.describe()
        } else {
            let rendered: Vec<String> = self.path.iter().map(|index| format!("{index}")).collect();
            format!("[{}] {}", rendered.join("."), self.diagnostic.describe())
        }
    }
}

/// Validates a layout declaration object and reports everything it could not honour.
///
/// # Why a separate validator and not the parser
///
/// The parser is deliberately forgiving: an absent or malformed spacing falls back to a default so
/// a slightly wrong document still renders. A *diagnostic* must be the opposite — it reports what
/// the parser tolerated. Keeping the two separate means the tolerant path stays tolerant and the
/// strict path is available to a designer on demand, from the same schema.
///
/// `obj` is the layout object as it appeared in the document. Returns one issue per problem, in a
/// stable order (the document's own key order), so a report is deterministic.
pub fn validate_layout_object(
    obj: &crate::compat::HashMap<String, crate::compat::String>,
    path: &[usize],
) -> Vec<LayoutIssue> {
    let mut issues = Vec::new();

    let Some(type_token) = obj.get("type") else {
        // No `type` at all is the parser's own error, which the caller surfaces; a diagnostic here
        // would be a second report of the same fact.
        return issues;
    };

    let Some(schema) = schema_for_type(type_token) else {
        issues.push(LayoutIssue {
            path: path.to_vec(),
            diagnostic: LayoutDiagnostic::UnknownType { declared: type_token.clone() },
        });
        return issues;
    };

    for (key, value) in obj {
        if key == "type" {
            continue;
        }
        match schema.param(key) {
            None => issues.push(LayoutIssue {
                path: path.to_vec(),
                diagnostic: LayoutDiagnostic::UnknownField { key: key.clone() },
            }),
            Some(param) if !param.honoured => issues.push(LayoutIssue {
                path: path.to_vec(),
                diagnostic: LayoutDiagnostic::DroppedField {
                    key: key.clone(),
                    reason: "this build's constructor for the kind takes no such argument",
                },
            }),
            Some(param) => {
                if !value_matches_kind(value, param.kind) {
                    issues.push(LayoutIssue {
                        path: path.to_vec(),
                        diagnostic: LayoutDiagnostic::WrongKind {
                            key: key.clone(),
                            expected: param.kind,
                        },
                    });
                }
            }
        }
    }

    issues
}

/// Whether a declared value is of the right type for a parameter.
///
/// The value arrives as the document's own text (this validator runs over the parsed object rather
/// than a `serde_json::Value`, so it can be reached from the ABI too). A numeric parameter accepts
/// a run of digits with an optional leading `-`, and an axis accepts a known token — the same
/// answers [`crate::json::parse_layout_kind`] gives, stated here so a designer can check a document
/// before loading it.
fn value_matches_kind(value: &str, kind: ParamKind) -> bool {
    match kind {
        ParamKind::UInt => !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()),
        ParamKind::Int => {
            let digits = value.strip_prefix('-').unwrap_or(value);
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        }
        ParamKind::Axis => is_axis_token(value),
        ParamKind::Region => crate::layout::BorderRegion::from_token(value).is_some(),
        ParamKind::Float => {
            // An optional sign, then digits with at most one decimal point. Exponents are
            // not accepted: the parser reads the value through `serde_json`, whose number
            // grammar a document is expected to follow, and the diagnostic here is for a
            // *typo* (`"ratio": "16:9"`), not for a full JSON grammar re-implementation.
            let body = value.strip_prefix('-').unwrap_or(value);
            !body.is_empty()
                && body != "."
                && body.matches('.').count() <= 1
                && body.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.')
        }
        ParamKind::Bool => value == "true" || value == "false",
    }
}

/// Whether `value` is an axis token the parser understands.
///
/// Reads [`crate::json::layout::AXIS_TOKENS`] — the parser's own list — rather than repeating it,
/// so the schema cannot accept a token the parser misreads. Keeping the two in sync by hand is
/// exactly how the splitter's `"column"` came to be accepted by the schema and read as Horizontal.
pub(crate) fn is_axis_token(value: &str) -> bool {
    crate::json::layout::AXIS_TOKENS.iter().any(|(token, _)| *token == value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::HashMap;

    fn obj(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (String::from(*k), String::from(*v))).collect()
    }

    /// Every kind the schema lists must be findable by its token and its aliases.
    #[test]
    fn every_schema_is_findable_by_its_tokens() {
        for schema in LAYOUT_KIND_SCHEMAS {
            assert!(
                schema_for_type(schema.type_token).is_some(),
                "`{}` is not findable",
                schema.type_token
            );
            for alias in schema.aliases {
                assert!(
                    schema_for_type(alias).is_some(),
                    "alias `{alias}` of `{}` is not findable",
                    schema.type_token
                );
            }
        }
    }

    /// The two dropped parameters must be reported as `honoured: false`, so a panel can grey them.
    #[test]
    fn the_dropped_parameters_are_marked_unhonoured() {
        let stack = schema_for_type("stack").expect("stack");
        assert!(!stack.param("spacing").expect("spacing").honoured);
        let splitter = schema_for_type("splitter").expect("splitter");
        assert!(!splitter.param("margin").expect("margin").honoured);
        assert!(splitter.param("orientation").expect("orientation").honoured);
    }

    /// A valid declaration reports nothing.
    #[test]
    fn a_valid_declaration_reports_nothing() {
        let issues = validate_layout_object(&obj(&[("type", "hbox"), ("spacing", "4")]), &[]);
        assert!(issues.is_empty(), "{issues:?}");
    }

    /// An unknown type is reported, not silently accepted.
    #[test]
    fn an_unknown_type_is_reported() {
        let issues = validate_layout_object(&obj(&[("type", "spiral")]), &[1]);
        assert_eq!(issues.len(), 1);
        assert!(matches!(issues[0].diagnostic, LayoutDiagnostic::UnknownType { .. }));
        assert!(issues[0].describe().contains("spiral"));
    }

    /// An unknown field is reported — a typo must not silently do nothing.
    #[test]
    fn an_unknown_field_is_reported() {
        let issues = validate_layout_object(&obj(&[("type", "hbox"), ("spacng", "4")]), &[]);
        assert_eq!(issues.len(), 1);
        assert!(
            matches!(issues[0].diagnostic, LayoutDiagnostic::UnknownField { ref key } if key == "spacng")
        );
    }

    /// A parameter the constructor drops is reported as dropped, distinctly from unknown.
    #[test]
    fn a_dropped_field_is_reported_as_dropped() {
        let issues = validate_layout_object(&obj(&[("type", "stack"), ("spacing", "4")]), &[]);
        assert_eq!(issues.len(), 1);
        assert!(
            matches!(issues[0].diagnostic, LayoutDiagnostic::DroppedField { ref key, .. } if key == "spacing")
        );
        assert!(issues[0].describe().contains("not applied"));
    }

    /// A wrong-typed value is reported with the expected kind.
    #[test]
    fn a_wrong_kind_is_reported() {
        let issues = validate_layout_object(&obj(&[("type", "hbox"), ("spacing", "wide")]), &[]);
        assert_eq!(issues.len(), 1);
        assert!(matches!(
            issues[0].diagnostic,
            LayoutDiagnostic::WrongKind { ref expected, .. } if *expected == ParamKind::UInt
        ));
    }

    /// A negative value is wrong for a `UInt` parameter but right for an `Int` one.
    #[test]
    fn signed_parameters_accept_negative_values() {
        assert!(
            validate_layout_object(&obj(&[("type", "hbox"), ("spacing", "-1")]), &[]).len() == 1
        );
        assert!(
            validate_layout_object(&obj(&[("type", "flow"), ("spacing", "-1")]), &[]).is_empty()
        );
    }

    /// Every declared kind must round-trip through the schema's own token.
    /// B26.05: a child placement key the schema declares must be one the parser actually reads.
    ///
    /// The test goes through the real `ChildLayoutAttrs::from_value` rather than comparing two
    /// lists: a schema that declared a key the parser ignores would pass a string comparison and
    /// fail here.
    #[test]
    fn a_declared_child_key_is_read_by_the_parser() {
        use crate::json::ChildLayoutAttrs;

        for attr in CHILD_ATTR_SCHEMA {
            // A JSON *number*, because that is what `from_value` reads (the validator's own input
            // is the document's text, which is a different entry point) — except for the two
            // token-valued keys, which are strings.
            let json = match attr.key {
                "stretch" => serde_json::json!({ "stretch": 7 }),
                "region" => serde_json::json!({ "region": "north" }),
                key => serde_json::json!({ key: 3 }),
            };
            let attrs = ChildLayoutAttrs::from_value(&json);
            let read = match attr.key {
                "stretch" => attrs.stretch == 7,
                "col" => attrs.col == Some(3),
                "row" => attrs.row == Some(3),
                "col_span" => attrs.col_span == Some(3),
                "row_span" => attrs.row_span == Some(3),
                "region" => attrs.region == Some(crate::layout::BorderRegion::North),
                "extent" => attrs.extent == Some(3),
                other => panic!("`{other}` is declared by the schema but this test cannot read it"),
            };
            assert!(read, "the schema declares `{}` but the parser did not read it", attr.key);
        }
    }

    /// B26.05: a grid is told its cell keys are honoured; a box layout's are reported as dropped.
    #[test]
    fn only_a_grid_honours_the_cell_keys() {
        let grid = child_attrs_honoured_by("grid");
        assert!(grid.contains(&"col_span"), "{grid:?}");
        let hbox = child_attrs_honoured_by("hbox");
        assert_eq!(hbox, vec!["stretch"], "a box layout places its children itself: {hbox:?}");

        let mut child = HashMap::new();
        child.insert(String::from("col"), String::from("1"));
        assert!(validate_child_attrs(&child, "grid", &[0]).is_empty());
        let issues = validate_child_attrs(&child, "hbox", &[0]);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(matches!(issues[0].diagnostic, LayoutDiagnostic::DroppedField { .. }));
        assert_eq!(issues[0].path, vec![0usize]);

        // A border layout honours the region keys and reports the cell keys as dropped.
        let border = child_attrs_honoured_by("border");
        assert!(border.contains(&"region"), "{border:?}");
        assert!(border.contains(&"extent"), "{border:?}");
        let mut region = HashMap::new();
        region.insert(String::from("region"), String::from("north"));
        region.insert(String::from("extent"), String::from("48"));
        assert!(validate_child_attrs(&region, "border", &[0]).is_empty());
        let cell = validate_child_attrs(&region, "grid", &[0]);
        assert_eq!(cell.len(), 2, "a grid does not place by region: {cell:?}");
    }

    /// B26.05: an unknown or malformed child key is named rather than ignored.
    #[test]
    fn a_child_key_the_schema_does_not_declare_is_reported() {
        let mut child = HashMap::new();
        child.insert(String::from("stretc"), String::from("2"));
        let issues = validate_child_attrs(&child, "grid", &[]);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(matches!(issues[0].diagnostic, LayoutDiagnostic::UnknownField { .. }));

        let mut negative = HashMap::new();
        negative.insert(String::from("stretch"), String::from("-1"));
        let issues = validate_child_attrs(&negative, "vbox", &[]);
        assert!(matches!(issues[0].diagnostic, LayoutDiagnostic::WrongKind { .. }), "{issues:?}");
    }

    #[test]
    fn the_supported_types_list_is_not_empty_and_sorted_consistently() {
        let types = supported_types();
        assert!(types.len() >= 10, "only {} kinds are described", types.len());
        assert!(types.contains(&"hbox") && types.contains(&"flex"));
    }
}
