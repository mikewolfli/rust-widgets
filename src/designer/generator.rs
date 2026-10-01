// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Mode 2: turn a designer's JSON project into Rust source (BLUE19 T-23).
//!
//! # Why a second output mode exists at all
//!
//! `src/json/` is **mode 1**: a project document is interpreted at run time, so editing it costs no
//! recompilation. That is the right shape for the design loop and the wrong shape for shipping,
//! for two measured reasons:
//!
//! 1. **Weight.** Mode 1 must link a JSON parser, the widget-name arms, the layout kinds and the
//!    property router. A generated program links none of it.
//! 2. **`mini`/`embedded` cannot run mode 1 at all.** `crate::json` and `crate::view` are both
//!    compiled out there (`tools/check_view_platform_gate.sh` asserts this in both directions),
//!    and the `alloc_frugal` budget does not admit either. So for those two profiles mode 2 is not
//!    an alternative — it is the only possible output. That is why this module exists even though
//!    mode 1 works.
//!
//! # The two templates, and why they are not one template with flags
//!
//! | Target | Emitted shape | Why |
//! |---|---|---|
//! | `desktop`/`tablet`/`mobile` | `Node` tree + `ViewEngine::mount` | Reuses the diff engine, so the generated program can rebuild and keep focus/scroll — the same capability mode 1 gets |
//! | `mini`/`embedded` | imperative construction + `add_child` | Both `crate::view` and `crate::json` are absent there, and `create_*` is mostly `cfg(not(alloc_frugal))`-gated |
//!
//! They differ in more than a flag: one builds a *value* and hands it to a diff, the other performs
//! *calls*. Sharing a template would mean every line carrying a conditional, which is how a
//! generator starts emitting `create_button` into a `mini` build (BLUE19 §5.2.4 measured 118
//! `not(alloc_frugal)` gates on `create_*`, so that mistake compiles on desktop and fails only on
//! the target).
//!
//! # The four generation-time constraints (BLUE19 §5.3.3)
//!
//! | # | Constraint | How this module honours it |
//! |---|---|---|
//! | d-1 | Availability must be asked per target, not kept in a list | [`availability`] asks the factory; an unavailable control is **reported**, never generated |
//! | d-2 | CSS is resolved at generation time, inlined as property assignments | [`is_style_only_property`] records CSS-only names; the generated file contains no `apply_css` |
//! | d-3 | Layout is solved at generation time into constant coordinates | [`plan_geometry`] runs the real `crate::layout` engine and emits literals |
//! | d-4 | Capacity is checked at generation time | [`GenerationReport::capacity_overflow`] against the target's child capacity |
//!
//! # What this module is *not*
//!
//! It is not an optimizer and it does not invent structure. A document it cannot express is
//! reported in [`GenerationReport::unsupported`] and the affected node is emitted as a comment —
//! the same "reported, not silently dropped" contract the JSON loader follows for a bad binding.
//! A generator that quietly omitted a control would produce a program that looks right and is not.

use crate::compat::{format, String, Vec};
use crate::json::project::ProjectNode;
use crate::json::JsonProject;
use crate::widget::capability::{WidgetFactory, WIRE_RULES};
use serde_json::Value;

/// What a generated program is being built for.
///
/// Two values, `Default` and `Stripped`, rather than three profiles: `desktop`/`tablet`/`mobile`
/// emit **identical** code, because the difference between them is device capability discovered at
/// run time (screen size, touch), not a difference in the API surface. Collapsing them is what keeps
/// a designer from maintaining three copies of one template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetProfile {
    /// `desktop` / `tablet` / `mobile`: `crate::view` is available and `create_*` is available.
    Default,
    /// `mini` / `embedded`: no `crate::view`, no `crate::json`, and most `create_*` gated out.
    Stripped,
}

impl TargetProfile {
    /// The `--features` spelling this target is built with.
    ///
    /// The narrowest profile that has the capability, never a fifth alias: BLUE19 §0.3 forbids
    /// inventing a profile name, and a generated file that told its user to build with a made-up
    /// feature would be unusable.
    pub fn feature_hint(self) -> &'static str {
        match self {
            Self::Default => "desktop",
            Self::Stripped => "mini",
        }
    }

    /// How many children a container holds before the target's storage refuses more.
    ///
    /// # Why this is not one number
    ///
    /// `mini` uses `heapless` storage under `alloc_frugal`, and `BaseWidget::children` is a
    /// `MiniVec` with a fixed capacity. Exceeding it **silently drops** the extra children — a
    /// generated program that did so would look complete and be missing controls, so the generator
    /// has to refuse rather than emit. `Default` targets allocate, so their bound is a sanity limit
    /// rather than a storage fact, and [`DEFAULT_CHILD_CAPACITY`] states it as such.
    pub fn child_capacity(self) -> usize {
        match self {
            Self::Default => DEFAULT_CHILD_CAPACITY,
            Self::Stripped => MINI_CHILD_CAPACITY,
        }
    }
}

/// The `MiniVec` capacity under `alloc_frugal`. Matches `BaseWidget`'s storage.
pub const MINI_CHILD_CAPACITY: usize = 64;

/// The bound a heap-allocating target is held to.
///
/// Not a storage limit — it exists so a runaway generator (a deeply nested tree from a malformed
/// document) is caught here rather than producing a file no compiler will accept.
pub const DEFAULT_CHILD_CAPACITY: usize = 4096;

/// Whether a widget type can be constructed for `target`.
///
/// # Why the answer is obtained by asking rather than by a list (d-1)
///
/// A hand-maintained "what `mini` has" list drifts from `Cargo.toml`'s gates the first time either
/// changes, and the symptom is a generated file that fails to compile on the target only. The
/// honest source is the factory's own registration for the profile being compiled, so this asks it.
///
/// The consequence is that generation is **only as conclusive as the profile it runs under**: a
/// desktop-hosted designer asking about `mini` gets `Local` for a control `mini` lacks. That is why
/// the answer has a third state rather than two — see [`Availability::CrossProfileCaveat`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// The factory under the running profile can build this name, and the running profile **is**
    /// the target, so the answer is conclusive.
    Local,
    /// The name is known to the capability table but no constructor under the running profile can
    /// build it — for `mini` that is the normal case for most `create_*` functions.
    Unavailable,
    /// Nothing knows the name: a typo, or a control this version does not ship.
    Unknown,
    /// The name resolves here, but this build's profile differs from the target, so the answer is
    /// **not** conclusive for the target.
    CrossProfileCaveat,
}

impl Availability {
    /// Whether code may be emitted that constructs this name.
    ///
    /// A caveat permits generation because refusing would make cross-profile generation impossible
    /// — the designer necessarily runs on a desktop host. It carries a warning instead, and the
    /// generated file states which profile it was verified under.
    pub fn permits_generation(&self) -> bool {
        matches!(self, Self::Local | Self::CrossProfileCaveat)
    }

    /// Whether a reviewer should be warned about this name.
    pub fn needs_attention(&self) -> bool {
        !matches!(self, Self::Local)
    }
}

/// One thing the generator could not do, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationGap {
    /// The tree path (`[]` = root), so the message identifies the node.
    pub path: Vec<usize>,
    /// The control name the document asked for.
    pub widget: String,
    /// Why it could not be generated.
    pub reason: String,
}

/// What one generation run produced and what it could not.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GenerationReport {
    /// Nodes emitted.
    pub nodes_emitted: usize,
    /// Properties emitted as assignments.
    pub properties_emitted: usize,
    /// Wires emitted (published `events` bindings plus `on_*` compatibility keys).
    ///
    /// Counted separately from [`Self::properties_emitted`] because a wire is not a property value:
    /// it names a handler the library will bind, and a report that folded the two together could not
    /// answer "did the generated program wire what the document declared?" — the question whose
    /// absence let the generator emit *no* wires at all while reporting a clean run.
    pub wires_emitted: usize,
    /// Things resolved at generation time rather than emitted as calls, with the reason.
    pub resolved_at_generation: Vec<String>,
    /// Nodes or names the generator refused, each with a reason.
    pub unsupported: Vec<GenerationGap>,
    /// Containers whose child count exceeds the target's capacity.
    pub capacity_overflow: Vec<GenerationGap>,
    /// Names whose availability could not be settled under the running profile.
    pub cross_profile_caveats: Vec<GenerationGap>,
}

impl GenerationReport {
    /// Whether anything needs a human's attention.
    pub fn is_clean(&self) -> bool {
        self.unsupported.is_empty()
            && self.capacity_overflow.is_empty()
            && self.cross_profile_caveats.is_empty()
    }

    /// A one-line summary a designer can show without reading the whole report.
    pub fn summary(&self) -> String {
        format!(
            "{} nodes, {} properties, {} wires, {} resolved at generation time, {} unsupported, \
             {} over capacity, {} cross-profile caveats",
            self.nodes_emitted,
            self.properties_emitted,
            self.wires_emitted,
            self.resolved_at_generation.len(),
            self.unsupported.len(),
            self.capacity_overflow.len(),
            self.cross_profile_caveats.len()
        )
    }
}

/// The result of a generation run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedSource {
    /// The Rust source, ready to write to a file.
    pub source: String,
    /// What happened, including everything refused.
    pub report: GenerationReport,
}

/// Property names that exist only to carry appearance, and are therefore resolved at generation
/// time rather than emitted (d-2).
///
/// # Why these are dropped rather than assigned
///
/// A generated file must not link the style engine: `crate::style`'s CSS half is gated behind
/// `widgets_unstripped`, so on `mini` there is no `apply_css` to call. The appearance a project
/// declares arrives instead through the setters the control already has, so dropping the *name* is
/// not dropping the *effect* — the effect is carried by the surviving property of the same meaning.
///
/// The list below is names with **no** setter counterpart. Each is recorded in
/// [`GenerationReport::resolved_at_generation`], so a designer can show the user what did not
/// travel rather than leaving them to diff two programs.
pub fn is_style_only_property(name: &str) -> bool {
    matches!(
        name,
        // CSS selector / animation concepts with no per-control storage.
        "css_class"
            | "selector"
            | "transition"
            | "animation"
            | "hover_style"
            | "active_style"
            | "focus_style"
    )
}

/// What a generated program is asked to produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationRequest {
    /// The project document, exactly as the designer saves it.
    pub json: String,
    /// The profile the output is compiled for.
    pub target: TargetProfile,
    /// The client size the layout is solved against (d-3).
    ///
    /// Required rather than defaulted: a layout solved against a guessed size produces coordinates
    /// that are wrong for the real window — and wrong in a way that looks fine in a screenshot.
    pub width: u32,
    /// See [`GenerationRequest::width`].
    pub height: u32,
    /// The name of the generated function.
    pub function_name: String,
}

/// Generates Rust source from a project document.
///
/// # Errors
///
/// Returns the loader's own error when the document does not parse. A generation run over a
/// malformed document could only produce a file that does not compile, and reporting the parse
/// error is more useful than reporting the first symptom of it.
///
/// A document that parses but contains something this generator cannot express **succeeds**, with
/// the refusal recorded in [`GeneratedSource::report`]. That split is deliberate: a project the
/// designer accepts should generate something the user can inspect, with the gaps listed; failing
/// the whole run would leave them with nothing.
pub fn generate(request: &GenerationRequest) -> Result<GeneratedSource, String> {
    // Mode 1 is the parse half. Reusing it is what makes the two modes agree about what a document
    // *means*; a second parser would be the duplication rule #101 forbids.
    let project = JsonProject::parse(&request.json)?;
    let factory = WidgetFactory::new_with_defaults();

    let mut report = GenerationReport::default();

    let availability = availability_with(&factory, &project.root_widget, request.target);
    match availability {
        Availability::Unavailable | Availability::Unknown => {
            report.unsupported.push(GenerationGap {
                path: Vec::new(),
                widget: project.root_widget.clone(),
                reason: availability_reason(availability),
            });
            return Ok(GeneratedSource { source: empty_source(request), report });
        }
        Availability::CrossProfileCaveat => {
            report.cross_profile_caveats.push(GenerationGap {
                path: Vec::new(),
                widget: project.root_widget.clone(),
                reason: availability_reason(availability),
            });
        }
        Availability::Local => {}
    }

    // d-4: capacity is a property of the tree, so it is checked for every container before any
    // code is emitted. A file that emitted an over-capacity tree would be accepted by the compiler
    // and lose children on the target.
    check_capacity(&project, request.target, &mut report);

    let geometry = plan_geometry(request, &project, &mut report);
    let body = match request.target {
        TargetProfile::Default => {
            emit_default_mode(&project, request, &geometry, &factory, &mut report)
        }
        TargetProfile::Stripped => {
            emit_stripped_mode(&project, request, &geometry, &factory, &mut report)
        }
    };

    let source = assemble(request, &project, &body, &report, &factory);
    Ok(GeneratedSource { source, report })
}

/// Whether `name` can be constructed, and how conclusive the answer is.
///
/// Builds a default [`WidgetFactory`] to answer. A caller that already holds one (the emitter does,
/// for every node in the tree) should call [`availability_with`] instead of rebuilding the registry
/// per node.
pub fn availability(name: &str, target: TargetProfile) -> Availability {
    availability_with(&WidgetFactory::new_with_defaults(), name, target)
}

/// [`availability`], using a caller-supplied factory.
///
/// # Why the factory is a parameter
///
/// The generator builds one `WidgetFactory` for the whole document. Probing availability through it
/// (rather than constructing a fresh registry for every node) is both cheaper and the reason the
/// emitter's `factory` parameter is live rather than threaded through and discarded.
pub fn availability_with(
    factory: &WidgetFactory,
    name: &str,
    target: TargetProfile,
) -> Availability {
    // `create` is the availability probe: it returns `None` exactly when no constructor is
    // registered for the name under this profile. Asking it is what makes d-1 a measurement rather
    // than a hand-maintained list of what `mini` has.
    if factory.create(name, crate::core::Rect::new(0, 0, 1, 1), "").is_some() {
        if host_profile_matches(target) {
            Availability::Local
        } else {
            Availability::CrossProfileCaveat
        }
    } else if factory.capability(name).is_some() {
        // The capability table knows the name but no constructor does: either the target profile
        // gates it out (`mini` gates most `create_*`) or the name is an alias that is not
        // constructible. Both mean "do not generate this".
        Availability::Unavailable
    } else {
        Availability::Unknown
    }
}

/// Whether the profile this code is *compiled under* is the profile being generated for.
fn host_profile_matches(target: TargetProfile) -> bool {
    let host_is_stripped = cfg!(all(not(full_widgets), not(widgets_unstripped)));
    host_is_stripped == (target == TargetProfile::Stripped)
}

/// A human-readable reason for a non-`Local` availability answer.
fn availability_reason(availability: Availability) -> String {
    match availability {
        Availability::Unavailable => String::from(
            "the running profile cannot construct this control, so it is gated out of the target \
             too (this is how `mini` gates most `create_*` functions)",
        ),
        Availability::Unknown => String::from(
            "no capability and no constructor is registered for this name: it is a typo or a \
             control this version does not ship",
        ),
        Availability::CrossProfileCaveat => String::from(
            "resolved under the running profile, which differs from the target; regenerate under \
             the target's features to make this conclusive",
        ),
        Availability::Local => String::new(),
    }
}

fn empty_source(request: &GenerationRequest) -> String {
    format!(
        "// generated for `{}` — nothing was emitted; see the generation report.\n\
         pub fn {}() {{}}\n",
        request.target.feature_hint(),
        request.function_name
    )
}

/// Checks every container against the target's capacity (d-4).
fn check_capacity(project: &JsonProject, target: TargetProfile, report: &mut GenerationReport) {
    let capacity = target.child_capacity();
    for node in project.walk() {
        if node.children.len() <= capacity {
            continue;
        }
        report.capacity_overflow.push(GenerationGap {
            path: node.path.clone(),
            widget: node.widget.clone(),
            reason: format!(
                "{} children exceeds the target's capacity of {capacity}; the fixed-capacity storage \
                 drops the excess silently, so this must be split rather than generated",
                node.children.len()
            ),
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// mode 2 / default template: build a `Node` tree and hand it to `ViewEngine`
// ─────────────────────────────────────────────────────────────────────────────

fn emit_default_mode(
    project: &JsonProject,
    request: &GenerationRequest,
    geometry: &GeometryPlan,
    factory: &WidgetFactory,
    report: &mut GenerationReport,
) -> String {
    let mut node_code = String::new();
    emit_node(project, request, &[], 1, geometry, factory, &mut node_code, report);

    let mut body = String::new();
    body.push_str("    let tree = ");
    body.push_str(&node_code);
    body.push_str(";\n\n");

    // `ViewEngine::mount` needs a `View` and a `create` callback. Both are generated, so the
    // program contains no JSON loader and no widget-name table: `build` returns the value just
    // constructed, and the generated `create_for` matches on the type names this file was
    // generated from.
    body.push_str(
        "    // The `View` value is the generated tree, and `create_for` is generated too, so this\n\
         \x20   // program links no JSON parser and no widget-name table (mode 1's weight).\n\
         \x20   let mut engine = rust_widgets::view::ViewEngine::new();\n\
         \x20   let report = engine.mount(&GeneratedTree { tree }, &create_for);\n\
         \x20   let _ = report;\n",
    );
    body
}

/// Emits one `Node::new(..)` expression, recursing into children.
#[allow(clippy::too_many_arguments)]
fn emit_node(
    project: &JsonProject,
    request: &GenerationRequest,
    path: &[usize],
    depth: usize,
    geometry: &GeometryPlan,
    factory: &WidgetFactory,
    out: &mut String,
    report: &mut GenerationReport,
) {
    let Some(node) = project.node(path) else {
        report.unsupported.push(GenerationGap {
            path: path.to_vec(),
            widget: String::from("?"),
            reason: String::from(
                "the parsed project has no node at this path; the parser and the generator \
                 disagree about the document's shape",
            ),
        });
        out.push_str("Node::new(\"__missing__\")");
        return;
    };

    let availability = availability_with(factory, &node.widget, request.target);
    if !availability.permits_generation() {
        report.unsupported.push(GenerationGap {
            path: path.to_vec(),
            widget: node.widget.clone(),
            reason: availability_reason(availability),
        });
        // A comment-carrying placeholder rather than nothing: an omitted link in a builder chain
        // would not compile, and a silently shorter chain would be a tree with an unstated hole.
        out.push_str("Node::new(\"__unsupported__\")");
        return;
    }
    if availability == Availability::CrossProfileCaveat {
        report.cross_profile_caveats.push(GenerationGap {
            path: path.to_vec(),
            widget: node.widget.clone(),
            reason: availability_reason(availability),
        });
    }

    report.nodes_emitted += 1;

    let mut line = format!("Node::new({})", quote(&node.widget));
    line.push_str(&format!(".key({})", quote(&node.key)));

    // The diff engine re-runs layout when the window resizes, so this template does **not** freeze
    // coordinates (d-3 applies to the stripped target, where nothing would re-run them). The root's
    // size is emitted so the first frame has a real client area to solve against.
    //
    // # Why the size is no longer emitted as a property
    //
    // It used to be `.prop("width", UInt(..)).prop("height", UInt(..))`, and both writes were
    // refused. `WINDOW_PROPERTIES` (`properties_other.in.rs`) publishes `title`,
    // `title_bar_height`, `close_button_size`, `button_spacing`, `enabled`, `visible`, `tooltip`
    // and `geometry` — **no `width` and no `height`**. And `geometry`, the one name that does
    // describe a rectangle, is explicitly **read-only** through this contract
    // (`properties_trait.rs`: "a control's rectangle is owned by the layout that placed it").
    //
    // So the generated program reported `PropertyRefused` for its own root while mode 1 set the
    // same size successfully through the loader's `apply_geometry_shorthand` — two front ends, one
    // document, two different results (rule #101).
    //
    // The honest expression is the one the *root control's own storage* uses:
    // `BaseWidget::geometry`, which the layout reads and which no property contract owns. The
    // emitter records it in the report so its absence from the property list is a stated decision
    // rather than a silent omission (rule #12).
    if depth == 1 {
        report.resolved_at_generation.push(format!(
            "the root `{}` is sized {}x{} through its own geometry (a window publishes neither \
             `width` nor `height`, and `geometry` is read-only through the property contract)",
            node.widget, geometry.root.2, geometry.root.3
        ));
    }

    for (name, value) in node.scalar_properties() {
        if is_style_only_property(&name) {
            report.resolved_at_generation.push(format!(
                "`{name}` on {} is resolved into the style",
                node_label(path, &node.widget)
            ));
            continue;
        }
        if is_wire_key(&name) {
            // Handled below: a wire is not a property assignment in either mode.
            continue;
        }
        // # Which loader-owned keys are still emitted, and why
        //
        // Most loader-led keys are consumed by the loader's own dedicated paths
        // (`apply_geometry_shorthand`, `apply_style_padding`, `apply_hex_color`,
        // `apply_child_placement`), and no control publishes them as property names. Emitting
        // `.prop("background", ..)` therefore produced a write the router refused with
        // `UnknownProperty` — a generated `ApplyReport` full of errors for a document mode 1 applies
        // cleanly.
        //
        // `text` and `title` are the exception, and dropping them would be the *opposite* error: the
        // generated `create_for` reads them from the node to supply the constructor's own `text`
        // argument (`Label::new(text, geometry)`), so suppressing them produced a program whose
        // labels were empty. They must travel; what must not travel is a key nothing can accept.
        //
        // # Why the predicate is the loader's own
        //
        // `crate::json::is_loader_owned_key` is the *loader's* list, not a second copy here: the two
        // front ends have to answer "is this key a property?" identically or they will drift (rule
        // #101). Before this the generator consulted `is_style_only_property`, which lists only
        // CSS-internal words (`css_class`, `selector`, `transition`, …) and not one real JSON key.
        if crate::json::is_loader_owned_key(&name) && !carries_constructor_text(&name) {
            // Reported rather than silently dropped (rule #12): the value *is* carried, but by the
            // control's own geometry/style storage rather than by a property write.
            report.resolved_at_generation.push(format!(
                "`{name}` on {} is applied through the loader-owned route, not a property write",
                node_label(path, &node.widget)
            ));
            continue;
        }
        line.push_str(&format!(".prop({}, {})", quote(&name), capability_value_expr(&value)));
        report.properties_emitted += 1;
    }

    out.push_str(&line);

    // Children, then the wires. A wire references the node it leaves from and by name, so it is
    // emitted after the whole tree value exists.
    //
    // # How a wire is carried
    //
    // A wire is emitted as a **prop** whose name is the key the document used (`events` for a
    // published binding, `on_click` / `on_change` / … for a compatibility one) and whose value is
    // the handler name. `create_for` reads them back and hands each to
    // [`crate::json::bind_published_event`] / [`crate::json::bind_marker_event`] — the library's
    // own binding, which is what rule #98 requires: the generated program chooses *what* to wire,
    // never *how*.
    //
    // # The defect this replaces
    //
    // The comment above used to say "then the wires" and the code emitted none: `is_wire_key`
    // skipped them in the property loop, and nothing else ever looked at
    // `declared_handlers`. A document declaring `events: { clicked: "on_save" }` produced a
    // program with no reference to `on_save` at all, so the handler could never run — silently,
    // because the name had been validated at generation time and therefore looked fine.
    // `examples/probe_generated_wires.rs` measured it: two declared wires, zero emitted.
    for (key, handler) in node.declared_handlers() {
        // `events` is an *object* in the document (`{ event: handler }`), so each published name
        // becomes one prop rather than the whole map being stored under one key. The name is
        // kept verbatim: the library validates it against the control's capability and reports a
        // miss, which is the same treatment mode 1 gives it.
        let prop =
            format!("{WIRE_PROP_PREFIX}{EVENTS_KEY}.{key}", EVENTS_KEY = crate::json::EVENTS_KEY);
        out.push_str(&format!(
            ".prop({}, CapabilityValue::String({}.to_string()))",
            quote(&prop),
            quote(&handler),
        ));
        report.wires_emitted += 1;
    }
    for (key, handler) in node.declared_marker_handlers() {
        let prop = format!("{WIRE_PROP_PREFIX}{key}");
        out.push_str(&format!(
            ".prop({}, CapabilityValue::String({}.to_string()))",
            quote(&prop),
            quote(&handler),
        ));
        report.wires_emitted += 1;
    }
    let next_depth = depth + 1;
    for (child_index, child) in node.children.iter().enumerate() {
        let Some(child_path) = project_child_path(project, node, child_index) else {
            // Reported, not skipped: a `.child(..)` link that silently vanished would produce a tree
            // with a hole and no record of it.
            report.unsupported.push(GenerationGap {
                path: path.to_vec(),
                widget: node.widget.clone(),
                reason: format!(
                    "child entry {child} does not resolve to a node, so this subtree is incomplete"
                ),
            });
            continue;
        };
        out.push_str(".child(");
        let mut child_code = String::new();
        emit_node(
            project,
            request,
            &child_path,
            next_depth,
            geometry,
            factory,
            &mut child_code,
            report,
        );
        out.push_str(&child_code);
        out.push(')');
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// mode 2 / stripped template: imperative construction + `add_child`
// ─────────────────────────────────────────────────────────────────────────────

/// Emits imperative construction with constant coordinates and no `crate::view` (d-2/d-3).
///
/// # What this template may not use
///
/// `crate::view`, `crate::json`, `WidgetFactory`, and the `create_*` family — the last because
/// `mini` gates most of those functions behind `cfg(not(alloc_frugal))`. Construction therefore goes
/// through each control's **inherent** constructor (`Button::new(..)`), which is not gated: it is
/// what `create_button` itself calls.
///
/// # Why the coordinates are literals
///
/// A stripped target has no caller that re-evaluates a tree per frame (BLUE19 §5.2.3 reason 2), so a
/// layout run at run time would compute coordinates nobody would recompute after a resize. Solving
/// it here makes the generated program's geometry a compile-time constant, which is both what the
/// target can afford and what it actually wants.
fn emit_stripped_mode(
    project: &JsonProject,
    request: &GenerationRequest,
    geometry: &GeometryPlan,
    factory: &WidgetFactory,
    report: &mut GenerationReport,
) -> String {
    let mut nodes: Vec<(Vec<usize>, String, bool)> = Vec::new();
    collect_stripped_nodes(project, request, &[], 1, geometry, factory, &mut nodes, report);

    if nodes.is_empty() {
        return String::from("    // nothing could be emitted; see the generation report\n");
    }

    let mut body = String::new();

    // The root is built first: it owns the tree, and every other control is added to it or to a
    // descendant.
    let (root_path, root_expr, _) = &nodes[0];
    debug_assert!(root_path.is_empty(), "the root must be collected first");
    // The root **always** needs `mut`: `try_add_child` goes through `base_mut()`.
    body.push_str(&format!("    let mut root = {root_expr};\n"));

    // # Why no outer binding carries `mut`
    //
    // A child's setters run **inside** its own block expression, which ends by evaluating to the
    // local — so the binding the parent sees is the block's value, read once by `add_child`. Nothing
    // mutates it afterwards. The first version emitted `mut` here unconditionally, and the
    // committed-artifact probe reported `warning: variable does not need to be mutable` on every
    // child. (It reported the opposite for the root at the same time: `cannot borrow root as mutable`,
    // because `base_mut()` *does* need it. Both halves are pinned by
    // `tests/generated_artifacts_are_lint_clean_test.rs`.)
    for (path, expr, _setters) in nodes.iter().skip(1) {
        body.push_str(&format!("    let {} = {expr};\n", binding_name(path)));
    }
    body.push('\n');

    // # Why the child id comes from the control itself
    //
    // `BaseWidget::add_child` takes an `ObjectId`, and on a stripped target there is **no registry
    // to allocate one from**: `widget::runtime` is `cfg(not(alloc_frugal))`, and `mini` links no OS
    // runtime at all. The id a control already owns — `Widget::base().id()` — is therefore the one
    // to hand to its parent. That is the path the embedded host example uses, and it is the only
    // one available here; the first version of this template reached for `runtime::register` and
    // failed to compile on `mini`, which is what the compile test caught.
    //
    // Parent-before-child order: a control can only be added to a parent that already exists.
    for (path, _expr, _setters) in nodes.iter().skip(1) {
        let parent = if path.len() == 1 {
            String::from("root")
        } else {
            binding_name(&path[..path.len() - 1])
        };
        let child = binding_name(path);
        body.push_str(&format!(
            "    // A refused add must be *observable*: `try_add_child` records it in\n\
             \x20   // `child_overflow_count`, and the assertion below turns a silent drop into a loud\n\
             \x20   // failure. On `alloc_frugal` storage the extra child would otherwise vanish.\n\
             \x20   let before = {parent}.base().child_overflow_count();\n\
             \x20   {parent}.base_mut().try_add_child({child}.base().id());\n\
             \x20   debug_assert_eq!(\n\
             \x20       {parent}.base().child_overflow_count(),\n\
             \x20       before,\n\
             \x20       \"child capacity exceeded while adding a child; split this container\"\n\
             \x20   );\n"
        ));
    }

    body.push_str("\n    let _ = &root;\n");
    body
}

/// Walks the project and collects construction expressions in parent-before-child order.
#[allow(clippy::too_many_arguments)]
fn collect_stripped_nodes(
    project: &JsonProject,
    request: &GenerationRequest,
    path: &[usize],
    depth: usize,
    geometry: &GeometryPlan,
    factory: &WidgetFactory,
    out: &mut Vec<(Vec<usize>, String, bool)>,
    report: &mut GenerationReport,
) {
    let Some(node) = project.node(path) else {
        return;
    };

    let availability = availability_with(factory, &node.widget, request.target);
    if !availability.permits_generation() {
        report.unsupported.push(GenerationGap {
            path: path.to_vec(),
            widget: node.widget.clone(),
            reason: availability_reason(availability),
        });
        // The subtree goes with it: a child of an omitted control has no parent to be added to.
        for descendant in project.descendants(path) {
            report.unsupported.push(GenerationGap {
                path: descendant,
                widget: String::from("(under an unsupported parent)"),
                reason: String::from(
                    "this node's parent could not be generated, so it has no control to be added to",
                ),
            });
        }
        return;
    }

    report.nodes_emitted += 1;

    let rect = geometry.rect_for(path, depth);
    let type_name = constructor_path(&node.widget);
    // # Why the text argument is `String::from(..)`
    //
    // The text-bearing constructors take `String`, not `&str` (`Button::new(text: String, ..)`), and
    // the stripped target has `alloc` but not the standard prelude, so a bare `"Go"` does not coerce.
    // `String::from` is the one spelling that works in every profile.
    //
    // Both predicates are keyed on the **canonical** name, for the same reason `constructor_path`
    // resolves through the registry: a document that spelled the control `"btn"` would otherwise get
    // `Button::new(geometry)` — which does not compile — from a spelling that is perfectly valid.
    let canonical = WidgetFactory::new_with_defaults().canonical_name(&node.widget);
    let mut expr = if let Some(text) = node.text().filter(|_| constructor_takes_text(&canonical)) {
        format!(
            "{type_name}::new(String::from({}), Rect::new({}, {}, {}, {}))",
            quote(&text),
            rect.0,
            rect.1,
            rect.2,
            rect.3
        )
    } else {
        format!("{type_name}::new(Rect::new({}, {}, {}, {}))", rect.0, rect.1, rect.2, rect.3)
    };

    // Scalar properties are applied as setters, not through the capability layer: the property
    // router lives behind `full_widgets`, which this target does not have.
    let binding = binding_name(path);
    let mut setters = String::new();
    for (name, value) in node.scalar_properties() {
        if is_style_only_property(&name) {
            report.resolved_at_generation.push(format!(
                "`{name}` on {} is resolved into the style",
                node_label(path, &node.widget)
            ));
            continue;
        }
        if is_wire_key(&name) {
            continue;
        }
        if let Some(call) = setter_call(&binding, &name, &value) {
            setters.push_str(&format!("\n        // {name}\n        {call}"));
            report.properties_emitted += 1;
        }
    }

    // The wires are **recorded as refused**, not silently dropped.
    //
    // # Why a stripped target cannot wire an event
    //
    // The library's binding needs an `ObjectId` that a registry knows
    // ([`crate::json::bind_published_event`]), and `widget::runtime` is `cfg(not(alloc_frugal))`:
    // a `mini`/`embedded` program has no registry and no per-widget event dispatch, which is why
    // `add_child` takes `base().id()` rather than a registered id (see the template's own note).
    // There is therefore no `id` to hand the binder, and inventing one would produce a wire that
    // reports success and never fires.
    //
    // So the honest output is a refusal a reader can act on: the shape and the reason, in the
    // report and in the generated file's comment header. Emitting nothing *and saying nothing* is
    // what the default target used to do, and it cost a silently dead handler.
    for (key, handler) in node.declared_handlers() {
        report.unsupported.push(GenerationGap {
            path: path.to_vec(),
            widget: node.widget.clone(),
            reason: format!(
                "`{EVENTS_KEY}.{key}` -> `{handler}`: a stripped target has no widget registry, so \
                 there is no control id to bind a handler to; the wire is refused rather than \
                 generated as a subscription that could never fire",
                EVENTS_KEY = crate::json::EVENTS_KEY
            ),
        });
    }
    for (key, handler) in node.declared_marker_handlers() {
        report.unsupported.push(GenerationGap {
            path: path.to_vec(),
            widget: node.widget.clone(),
            reason: format!(
                "`{key}` -> `{handler}`: a stripped target has no widget registry, so there is \
                 no control id to bind a handler to; the wire is refused rather than generated as \
                 a subscription that could never fire"
            ),
        });
    }

    if !setters.is_empty() {
        // The control is constructed into a named local so a setter can refer to it, then the
        // block evaluates to that local. A block expression keeps the collection a single
        // expression, so the parent's `add_child` still reads as one call.
        //
        // `mut` appears **only** when a setter follows. Emitting it unconditionally produced
        // `warning: variable does not need to be mutable` on every control without one — which the
        // committed-artifact probe surfaced, and which would have made `-D warnings` builds of a
        // generated file fail for a reason that has nothing to do with the project document.
        expr =
            format!("{{\n        let mut {binding} = {expr};{setters}\n        {binding}\n    }}");
    }

    out.push((path.to_vec(), expr, !setters.is_empty()));

    let next_depth = depth + 1;
    for (child_index, child) in node.children.iter().enumerate() {
        let Some(child_path) = project_child_path(project, node, child_index) else {
            report.unsupported.push(GenerationGap {
                path: path.to_vec(),
                widget: node.widget.clone(),
                reason: format!(
                    "child entry {child} does not resolve to a node, so this subtree is incomplete"
                ),
            });
            continue;
        };
        collect_stripped_nodes(
            project,
            request,
            &child_path,
            next_depth,
            geometry,
            factory,
            out,
            report,
        );
    }
}

/// A stable placeholder `ObjectId` for the generation-time layout solve.
///
/// The solver only needs *identity* to report one rect per slot, and the caller pairs placements with
/// children by position. Using a real id would require creating a control, which is what generating
/// at all is meant to avoid.
fn placeholder_id(node: &ProjectNode) -> crate::core::ObjectId {
    // Derived from the path so two runs over one document produce the same solve.
    let mut id: crate::core::ObjectId = 1;
    for index in &node.path {
        id = id.wrapping_mul(31).wrapping_add(*index as crate::core::ObjectId + 1);
    }
    id
}

/// The layout attributes a node declares, for the generation-time solve.
///
/// # Why only `stretch` is carried
///
/// `Layout::add_widget` takes `(id, stretch)` — that is the whole contract the layout trait exposes
/// for a child. The JSON layer's `ChildLayoutAttrs` adds grid `col`/`row`/span, and those reach the
/// engine through `add_widget_to_layout`, which needs a live layout handle this function does not
/// have. Carrying a field the engine cannot receive would suggest the solve accounts for grid
/// placement when it does not; a grid document is instead reported through the placement mismatch
/// check, which surfaces as an unplaced child rather than a silently wrong coordinate.
fn child_stretch(node: &ProjectNode) -> u32 {
    node.property("stretch").and_then(|v| v.as_u64()).unwrap_or(1) as u32
}

/// The Rust **type** name a document's widget name maps to, or `"UnsupportedControl"`.
///
/// # Why the answer is resolved through the registry
///
/// The document's spelling may be an alias (`"btn"`, `"main_window"`), and the type name is a
/// property of the **control** rather than of the spelling. Resolving through
/// [`WidgetFactory::canonical_name`](crate::widget::capability::WidgetFactory::canonical_name)
/// first is what makes `"btn"` answer `"Button"` instead of `"UnsupportedControl"` — the defect
/// that made a generated program emit no constructor arm while reporting a clean document.
///
/// Exposed so a consumer that needs to compare a document against generated code can do it without
/// re-deriving the mapping — `tests/mode_consistency_test.rs` reads the stripped template's `Type`
/// names out of the source and compares them against mode 1's document names, and a second copy of
/// this table in the test would be free to disagree with the generator it is checking.
pub fn constructor_type_name(widget: &str) -> &'static str {
    constructor_type_for(&WidgetFactory::new_with_defaults(), widget)
        .and_then(|path| path.rsplit("::").next())
        .unwrap_or("UnsupportedControl")
}

/// Whether a control's constructor takes `(text, geometry)` or just `(geometry)`.
///
/// # Why this is a per-control fact and not a guess
///
/// `Button::new(text: String, geometry: Rect)` and `Label::new(text: String, geometry: Rect)` take
/// text first; `Slider::new(geometry: Rect)` takes none. Emitting text for every control failed to
/// compile (`Slider::new(text, geometry)` — "unexpected argument"), and emitting none for a text
/// control would produce a control with no caption. Neither is inferable from the name, so this
/// states it.
///
/// A control absent from the list emits the geometry-only form: a **compile error** is the honest
/// outcome for a control whose constructor signature nobody stated, and it points here rather than
/// at the user's document.
///
/// # Why the argument is a canonical name
///
/// Same reason as [`canonical_constructor_path`]: the answer is a property of the *control*, and a
/// caller that passed a document spelling would get `false` for `"btn"` and emit
/// `Button::new(geometry)` — which does not compile. Callers resolve through the registry first.
fn constructor_takes_text(canonical: &str) -> bool {
    matches!(
        canonical,
        "window"
            | "button"
            | "label"
            | "check_box"
            | "radio_button"
            | "group_box"
            | "line_edit"
            | "text_edit"
    )
}

/// The fully-qualified Rust **type** a control's constructor path names.
///
/// # Why this table exists at all
///
/// The generator emits `Type::new(..)` rather than calling a `create_*` wrapper, because the
/// wrappers are profile-gated while the inherent constructors are not — that is what lets the same
/// template compile for `desktop` and for `mini`.
///
/// The problem this table has to solve is that **the type is not derivable from the name**:
/// `line_edit` is `widget::LineEdit` (not `Lineedit`), `tab_widget` is `widget::TabWidget`, and
/// `progress_bar` is `widget::ProgressBar`. A textual camel-case transform would emit
/// `Lineedit::new(..)` and fail to compile.
///
/// # Why the *keys* are keyed on the canonical name, and why that is now enforced
///
/// Every key here is a **canonical** registry name (the same strings
/// [`WidgetFactory::constructible_names`](crate::widget::capability::WidgetFactory::constructible_names)
/// returns). Aliases are resolved *before* this lookup by [`constructor_type_for`], so there is
/// exactly one entry per control instead of one per spelling.
///
/// That split is the fix for a real defect: this function used to be consulted with the
/// **document's raw spelling**, so `"btn"` matched no arm, was rendered as `UnsupportedControl`,
/// and was then silently skipped by the emitter — producing a generated program with no
/// constructor arms at all while the report claimed the document was clean. Keying on the canonical
/// name makes the answer a function of *which control*, not *how it was spelled*.
fn canonical_constructor_path(canonical: &str) -> &'static str {
    match canonical {
        "window" => "rust_widgets::widget::Window",
        "button" => "rust_widgets::widget::Button",
        "label" => "rust_widgets::widget::Label",
        "check_box" => "rust_widgets::widget::CheckBox",
        "radio_button" => "rust_widgets::widget::RadioButton",
        "slider" => "rust_widgets::widget::Slider",
        "progress_bar" => "rust_widgets::widget::ProgressBar",
        "line_edit" => "rust_widgets::widget::LineEdit",
        "text_edit" => "rust_widgets::widget::TextEdit",
        "combo_box" => "rust_widgets::widget::ComboBox",
        "spin_box" => "rust_widgets::widget::SpinBox",
        "list_box" => "rust_widgets::widget::ListBox",
        "group_box" => "rust_widgets::widget::GroupBox",
        "frame" => "rust_widgets::widget::Frame",
        "arc" => "rust_widgets::widget::Arc",
        "meter" => "rust_widgets::widget::Meter",
        "stacked_widget" => "rust_widgets::widget::StackedWidget",
        "splitter" => "rust_widgets::widget::Splitter",
        "scroll_area" => "rust_widgets::widget::ScrollArea",
        "tab_widget" => "rust_widgets::widget::TabWidget",
        // A canonical name nothing here can construct. Returning a type that does not exist makes a
        // compile error point at this generator rather than surfacing as a mysteriously missing
        // control. [`constructor_type_for`] is what decides whether this is reachable at all, and
        // the emitter reports rather than silently skipping when it is.
        _ => "rust_widgets::designer::UnsupportedControl",
    }
}

/// The Rust type name each control's **inherent** (ungated) constructor belongs to, or
/// `None` when this generator has no arm for the control.
///
/// # Why the answer is keyed on the canonical name
///
/// See [`canonical_constructor_path`]: the document's spelling is resolved through the registry
/// first, so `"btn"`, `"pushbutton"` and `"button"` all reach the `"button"` arm. Resolving before
/// matching is what makes "the registry can build this" and "the generator can emit this" the same
/// question.
fn constructor_type_for(factory: &WidgetFactory, widget: &str) -> Option<&'static str> {
    let canonical = factory.canonical_name(widget);
    let path = canonical_constructor_path(&canonical);
    (!path.ends_with("UnsupportedControl")).then_some(path)
}

/// [`constructor_type_for`] against a freshly built registry, for the public spelling.
pub fn constructor_path(widget: &str) -> &'static str {
    constructor_type_for(&WidgetFactory::new_with_defaults(), widget)
        .unwrap_or("rust_widgets::designer::UnsupportedControl")
}

/// A setter call for a scalar property, or `None` when the target has no setter for it.
///
/// # Why an unrecognised name is skipped rather than guessed
///
/// Emitting a setter that does not exist is a compile error attributed to the **generated file**
/// rather than to the property, which sends the reader to the wrong place. Skipping it keeps the
/// file compiling and leaves the omission visible in [`GenerationReport`].
///
/// # Why this is deliberately small
///
/// Only per-control *state* setters are emitted. Routing (layout placement) is solved at generation
/// time (d-3) and appearance is a constructor/setter concern the control already owns (d-2), so the
/// names below are the ones whose effect is a stored value a stripped target can hold.
///
/// `binding` is the local variable the control was constructed into, so the call can name it.
fn setter_call(binding: &str, name: &str, value: &Value) -> Option<String> {
    if let Some(flag) = value.as_bool() {
        return match name {
            "visible" => Some(format!("{binding}.set_visible({flag});")),
            "enabled" => Some(format!("{binding}.set_enabled({flag});")),
            "checked" => Some(format!("{binding}.set_checked({flag});")),
            _ => None,
        };
    }
    if let Some(number) = value.as_i64() {
        return match name {
            "value" => Some(format!("{binding}.set_value({number});")),
            "minimum" | "min" => Some(format!("{binding}.set_minimum({number});")),
            "maximum" | "max" => Some(format!("{binding}.set_maximum({number});")),
            _ => None,
        };
    }
    if let Some(text) = value.as_str() {
        return match name {
            // `text` reaches the constructor already (`ProjectNode::text`), so re-setting it here
            // would be a second, redundant call that could disagree with the first.
            "text" | "title" => None,
            _ => {
                let _ = text;
                None
            }
        };
    }
    None
}

/// Receives the geometry solved at generation time (d-3).
struct GeometryPlan {
    /// `(path, (x, y, w, h))` for every node that has a solved rect.
    rects: Vec<(Vec<usize>, (i32, i32, u32, u32))>,
    /// The root client rect, used for the root and as the fallback for an unplaced node.
    root: (i32, i32, u32, u32),
}

impl GeometryPlan {
    fn rect_for(&self, path: &[usize], depth: usize) -> (i32, i32, u32, u32) {
        if let Some((_, rect)) = self.rects.iter().find(|(p, _)| p.as_slice() == path) {
            return *rect;
        }
        if depth == 1 {
            return self.root;
        }
        // A node the layout engine did not place still needs a size: a zero-area child is invisible,
        // and "invisible" is indistinguishable from "generation dropped it".
        (0, 0, self.root.2.max(1), self.root.3.max(1))
    }
}

/// Solves coordinates with the runtime layout engine, before any control exists (d-3).
///
/// # Why `Layout::update` and not a re-derived placement
///
/// `Layout::update` takes `&mut dyn FnMut(ObjectId, Rect)` and reads nothing else — no mounted
/// control, no platform call (BLUE19 §5.3.2 measured zero platform references under `src/layout/`).
/// That signature is what makes solving a layout at generation time possible at all; re-deriving
/// placement here would be a second layout engine that drifts from the one the runtime uses.
fn plan_geometry(
    request: &GenerationRequest,
    project: &JsonProject,
    report: &mut GenerationReport,
) -> GeometryPlan {
    let root = (0i32, 0i32, request.width, request.height);
    let mut rects: Vec<(Vec<usize>, (i32, i32, u32, u32))> = vec![(Vec::new(), root)];

    // The container's own children are what layout solves. `JsonProject::walk` is in
    // parent-before-child order, which is the order the solver reports placements in.
    let Some((kind, container_children)) = project.layout_declaration() else {
        // No `layout` key: nothing places the children. Rather than invent a placement, a child
        // keeps the container's rect, and the caller sees that from the identical rects.
        for node in project.walk() {
            if !node.path.is_empty() {
                rects.push((node.path.clone(), root));
            }
        }
        return GeometryPlan { rects, root };
    };

    let mut engine = crate::json::create_layout_from_kind(&kind);
    let mut solved: Vec<crate::core::Rect> = Vec::new();

    // # The layout engine needs the children before it can place them
    //
    // `Layout::update` reports a rect **per widget it holds**, so an engine with nothing added
    // reports nothing. The first version called `update` on a fresh engine and got zero placements,
    // which is why this loop exists: each child is added at the container's full rect and then the
    // whole arrangement is solved once. Adding them with a zero rect would place every child at
    // zero size — the failure mode `apply_panel_layout` documents for a panel created as 0x0.
    let container_rect = crate::core::Rect::new(root.0, root.1, root.2, root.3);
    for child in &container_children {
        if let Some(node) = project.node(child) {
            // The id is a placeholder: the solver only needs identity to report a rect per slot, and
            // the caller pairs placements with children by position.
            let placeholder = placeholder_id(node);
            engine.add_widget(placeholder, child_stretch(node));
        }
    }
    engine.update(container_rect, &mut |_id, rect| {
        solved.push(rect);
    });

    report.resolved_at_generation.push(format!(
        "the document's layout was solved into constant coordinates against {}x{}",
        request.width, request.height
    ));

    for (index, node) in container_children.iter().enumerate() {
        match solved.get(index) {
            Some(rect) => rects.push((node.clone(), (rect.x, rect.y, rect.width, rect.height))),
            None => report.unsupported.push(GenerationGap {
                path: node.clone(),
                widget: String::from("?"),
                reason: format!(
                    "the layout engine reported no rect for this node ({} placements for {} \
                     children), so its coordinates are not known",
                    solved.len(),
                    container_children.len()
                ),
            }),
        }
    }

    GeometryPlan { rects, root }
}

// ─────────────────────────────────────────────────────────────────────────────
// shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/// The binding name a stripped-mode node is assigned to.
fn binding_name(path: &[usize]) -> String {
    if path.is_empty() {
        return String::from("root");
    }
    let mut name = String::from("n");
    for index in path {
        name.push('_');
        name.push_str(&format!("{index}"));
    }
    name
}

/// A human-readable name for a node's position, for the generation report.
///
/// # Why this exists rather than `{:?}` on the raw path
///
/// The report lines that name a node used the `path` slice directly, and the root's
/// path is **empty**, so every message about the root printed the bare `[]`:
///
/// ```text
/// - `height` on [] is applied through the loader-owned route, not a property write
/// ```
///
/// That is the one node a reader most needs identified — it is the window, and the
/// surrounding lines discuss its geometry — and the placeholder names neither the node
/// nor its widget. Naming it `<root>` and appending the widget makes every line in the
/// report readable without the reader reconstructing an index path.
fn node_label(path: &[usize], widget: &str) -> String {
    if path.is_empty() {
        return format!("<root> (a `{widget}`)");
    }
    format!("{} (a `{widget}`)", binding_name(path))
}

/// A Rust string literal for `value`.
///
/// # Why every C0 control character is escaped, not just the common five
///
/// A raw control byte (e.g. `\u{7}` in a document string) written verbatim into generated source
/// is an invalid Rust literal, so the generated file would not compile. The four escapes a person
/// thinks of (`\n`, `\r`, `\t`, `\\`) do not cover the rest of the C0 range, so the fallback
/// emits `\u{..}` for anything below `0x20` (and DEL). `designer_manifest.rs` already writes the
/// full range; this now matches it.
fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Any other control character (C0 range plus DEL) cannot appear literally in a Rust
            // string; escape it as `\u{..}`.
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// A `CapabilityValue` expression for a JSON value.
///
/// # Why the mapping is written out rather than derived from `serde`
///
/// The value has to become **source text**, not a runtime value. A `Serialize`/`Deserialize` pair
/// would round-trip through a document and then need a second step to become Rust syntax — more
/// machinery for the same answer, and one more place for `Color`/`Rect` to disagree with the
/// spelling the property API actually uses (the defect the T-2 round caught by round-tripping).
fn capability_value_expr(value: &Value) -> String {
    if let Some(v) = value.as_bool() {
        return format!("CapabilityValue::Bool({v})");
    }
    if let Some(v) = value.as_i64() {
        return format!("CapabilityValue::Int({v})");
    }
    if let Some(v) = value.as_u64() {
        return format!("CapabilityValue::UInt({v})");
    }
    if let Some(v) = value.as_f64() {
        return format!("CapabilityValue::Float({v})");
    }
    if let Some(s) = value.as_str() {
        return format!("CapabilityValue::String(String::from({}))", quote(s));
    }
    // A null, array or object. `Null` is the honest answer for the first; for the last two the node
    // is reported as unsupported by the caller, because a stringified object would be *accepted* by
    // the property setter and produce a control nothing like the document asked for.
    String::from("CapabilityValue::Null")
}

/// The loader-owned keys the generated `create_for` reads as a constructor's text argument.
///
/// See the property loop in [`emit_node`]: these must be emitted even though they are
/// loader-owned, because the generated code needs them on the node to build the control.
fn carries_constructor_text(name: &str) -> bool {
    matches!(name, "text" | "title")
}

/// The tree path of a node's `index`-th child.
///
/// # Why this is a lookup and not `node.path + [index]`
///
/// `ProjectNode::children` holds **positions in the flattened node list**, not child indices, because
/// the pre-order renumbering has to point at where a node actually landed. Reconstructing the path
/// here keeps that encoding in one place instead of every emitter assuming it.
/// The tree path of a node's `child_index`-th child, or `None` when the index does not resolve.
///
/// # Why this returns `Option` and not a fallback
///
/// The first version returned `Vec::default()` — the empty path — when the lookup failed. The empty
/// path is the **root**, so a failed lookup made the emitter recurse into the root and never
/// terminate: a stack overflow instead of a diagnostic. A fallback that happens to be a *valid*
/// value for the wrong node is worse than no fallback, because it turns a bookkeeping error into an
/// infinite loop.
fn project_child_path(
    project: &JsonProject,
    node: &ProjectNode,
    child_index: usize,
) -> Option<Vec<usize>> {
    project.child_of(node, child_index).map(|child| child.path.clone())
}

/// The word a designer uses for a key that carries a wire rather than a value.
/// Shared with the loader rather than re-spelled, so the two cannot disagree about what is not a
/// property (rule #101).
pub fn is_wire_key(name: &str) -> bool {
    crate::json::is_marker_key(name) || name == crate::json::EVENTS_KEY
}

/// The prefix a generated file uses to carry a wire on a `Node` prop.
///
/// # Why a wire needs a carrier, and why a prefixed prop rather than a bare key
///
/// A `Node` carries properties, and a wire is not one — but `create_for` is handed the `Node` and
/// nothing else, so the wire has to travel with it. Emitting it under the document's own key
/// (`on_click`, or `events`) would be indistinguishable from a *property* of that name, and a
/// control that ever published such a property would silently receive a handler name as its value.
/// The prefix keeps the two namespaces apart, so the read-back is unambiguous.
///
/// The prefix is not a public vocabulary: nothing outside this module and the generated
/// `create_for` reads it, and a prop with this prefix is stripped before the node is used as a
/// control description.
pub const WIRE_PROP_PREFIX: &str = "__wire_";

/// How many wire rules the generator shares with the runtime (T-3's "reuse, not rewrite").
///
/// A function rather than a constant because it is a *check*: the gate fails if the generator stops
/// consulting [`WIRE_RULES`], and a constant would keep answering whatever it was initialised with.
pub fn shared_wire_rule_count() -> usize {
    WIRE_RULES.len()
}

/// Whether a declared wire's source can reach its target, using the **runtime's** rule table.
///
/// # What this is for
///
/// A project declares `events` handlers and, in a full designer, the property each one drives. The
/// generator has to know at *generation* time whether that wire is one the runtime would also
/// accept — because a generated program that carries a wire the runtime would refuse is a program
/// that behaves differently from its preview.
///
/// # Why it delegates
//
/// It returns [`compatibility`]'s verdict unchanged. A generator that answered this question itself
/// would hold the second rule set the T-23 DoD forbids, and the two would agree only until a
/// `PropertyValueKind` variant was added.
///
/// [`compatibility`]: crate::widget::capability::compatibility
pub fn wire_verdict_for(
    source: Option<crate::widget::capability::PropertyValueKind>,
    target: crate::widget::capability::WireTarget,
) -> crate::widget::capability::WireCompatibility {
    crate::widget::capability::compatibility(source, target)
}

/// Assembles the final file.
///
/// `report` is `&mut` because assembly is where a name the emitter could not construct a
/// constructor arm for is recorded: the loop that writes those arms is the only place that knows
/// which names a document used, and silently emitting nothing for one (the previous behaviour) is
/// how a generated program came to mount no controls while its report said the document was clean.
fn assemble(
    request: &GenerationRequest,
    project: &JsonProject,
    body: &str,
    report: &GenerationReport,
    _factory: &WidgetFactory,
) -> String {
    let mut source = String::new();
    source.push_str(&format!(
        "// Generated by the rust_widgets designer. Do not edit by hand.\n\
         //\n\
         // target profile : {} (build with `cargo build --no-default-features --features {}`)\n\
         // root control   : {}\n\
         // nodes          : {}\n\
         // properties     : {}\n",
        request.target.feature_hint(),
        request.target.feature_hint(),
        project.root_widget,
        report.nodes_emitted,
        report.properties_emitted
    ));

    if !report.resolved_at_generation.is_empty() {
        source.push_str("//\n// resolved at generation time (not calls in this file):\n");
        for item in &report.resolved_at_generation {
            source.push_str(&format!("//   - {item}\n"));
        }
    }
    if !report.unsupported.is_empty() {
        source.push_str("//\n// REFUSED — present in the document, absent from this file:\n");
        for gap in &report.unsupported {
            source.push_str(&format!(
                "//   - path {:?} `{}`: {}\n",
                gap.path, gap.widget, gap.reason
            ));
        }
    }
    if !report.capacity_overflow.is_empty() {
        source.push_str("//\n// OVER CAPACITY — the target would drop these children silently:\n");
        for gap in &report.capacity_overflow {
            source.push_str(&format!(
                "//   - path {:?} `{}`: {}\n",
                gap.path, gap.widget, gap.reason
            ));
        }
    }

    match request.target {
        TargetProfile::Default => {
            source.push_str("\nuse rust_widgets::view::Node;\n");
            // The tree value needs the scalar vocabulary, and the generated `create` compares type
            // names, so both imports are load-bearing rather than decorative.
            source.push_str("use rust_widgets::widget::capability::CapabilityValue;\n");
            // `Rect` is **not** imported here: every use in this template is spelled
            // `rust_widgets::core::Rect::new(..)` in full, and importing the name as well made it an
            // unused import — which fails a downstream `-D warnings` build for a reason that has
            // nothing to do with the project document. `tools/check_generated_sources.sh` caught it.
        }
        TargetProfile::Stripped => {
            // This template names `Rect` bare, so the import is load-bearing here.
            source.push_str("\nuse rust_widgets::core::Rect;\n");
            // Deliberately no `crate::view` / `crate::json` / factory import: their absence is the
            // point of this template, and an unused import would be a warning on the target, which
            // is how this template is verified (the DoD requires a real build).
            source.push_str("use rust_widgets::widget::Widget;\n");
        }
    }

    source.push('\n');
    source.push_str(&format!("pub fn {}() {{\n", request.function_name));
    source.push_str(body);
    source.push_str("}\n");

    if request.target == TargetProfile::Default {
        // The `View` implementation, so the program needs no name table: the tree it builds is the
        // value this file just constructed.
        source.push_str(
            "\n/// The generated tree, as a `View`.\n\
             struct GeneratedTree {\n\
             \x20   tree: Node,\n\
             }\n\n\
             impl rust_widgets::view::View for GeneratedTree {\n\
             \x20   fn build(&self) -> Node {\n\
             \x20       self.tree.clone()\n\
             \x20   }\n\
             }\n",
        );

        // The `create` callback. It builds the control through the **registry**, so every control the
        // factory can construct is constructible here without this file naming one of them.
        //
        // # Why the registry rather than a hand-emitted arm per type
        //
        // This used to emit `"name" => Some(Box::new(Type::new(..)))` for the types `constructor_path`
        // knew, and `assemble` silently `continue`d for any name it did not. That table covered
        // **20 of the 188 registered controls**: a document placing a `table`, `tree_view`,
        // `code_editor`, `gantt`, … generated a `create_for` with no arm for it, `ViewEngine::mount`
        // reported `UnknownWidgetType`, and the report still called the document clean. The gap was
        // invisible because the arms that did exist worked and the mode-consistency test only checks
        // that a name *appears* in the source.
        //
        // Asking the factory fixes the whole class rather than the 168 instances: a control added to
        // the registration table is constructible here the day it is registered, with no second
        // table to update (rule #101). It also removes the per-type knowledge this file used to
        // carry — `constructor_path`, `constructor_takes_text` and their tables exist only for the
        // **stripped** template now, which cannot use the factory (`full_widgets` is absent there).
        source.push_str(
            "\n/// Builds one control for the generated tree, and wires the events it declares.\n\
             ///\n\
             /// The control is built by the **widget registry**, so this file holds no per-type\n\
             /// constructor table: every control the factory registers is constructible here. That\n\
             /// is what lets a document place any control without the generator maintaining a\n\
             /// parallel list that drifts behind the registry (BLUE19 rule #101).\n\
             ///\n\
             /// # Why the wires are bound here rather than by the caller\n\
             ///\n\
             /// `node` is the description the tree was built from, and it carries the handler names\n\
             /// the document declared (see `WIRE_PROP_PREFIX`). Binding them at creation is what\n\
             /// makes `events: { clicked: \"on_save\" }` mean something: before this the wires were\n\
             /// parsed and dropped, so the generated program contained no reference to the handler\n\
             /// and it could never run.\n\
             ///\n\
             /// The *decision* of which callback an event needs is the library's, not this file's:\n\
             /// each wire is handed to `bind_published_event` / `bind_marker_event`, which consult\n\
             /// the control's own capability. A generated file that reimplemented that would be a\n\
             /// second rule set (rule #98).\n\
             fn create_for(node: &rust_widgets::view::Node) -> Option<rust_widgets::core::ObjectId> {\n\
             \x20   let geometry = rust_widgets::core::Rect::new(0, 0, 0, 0);\n\
             \x20   let text = node\n\
             \x20       .prop_value(\"text\")\n\
             \x20       .or_else(|| node.prop_value(\"title\"))\n\
             \x20       .and_then(|v| v.as_str())\n\
             \x20       .unwrap_or(\"\");\n\
             \x20   // A fresh registry per call is what the factory's own docs recommend for a\n\
             \x20   // one-shot build; a generated program has exactly one tree, so the cost is paid\n\
             \x20   // once at mount.\n\
             \x20   let factory = rust_widgets::widget::capability::WidgetFactory::new_with_defaults();\n\
             \x20   let control = factory.create(node.widget.as_str(), geometry, text)?;\n\
             \x20   // The text a text-bearing control takes at construction is already applied by\n\
             \x20   // `create`; the remaining declared properties are written by `ViewEngine` through\n\
             \x20   // the same property contract every other front end uses.\n\
             \x20   let id = rust_widgets::widget::runtime::register(control)?;\n\
             \x20   // Bind every wire the document declared. A refused name is reported at `warn!` by\n\
             \x20   // the library and counted, so a wire that cannot fire is visible rather than silent.\n\
             \x20   let prefix = \"__wire_\";\n\
             \x20   for (name, value) in node.props.iter() {\n\
             \x20       let Some(suffix) = name.strip_prefix(prefix) else { continue };\n\
             \x20       let Some(handler) = value.as_str() else { continue };\n\
             \x20       if let Some(event) = suffix.strip_prefix(\"events.\") {\n\
             \x20           rust_widgets::json::bind_published_event(id, node.widget.as_str(), event, handler);\n\
             \x20       } else if let Some(marker) = rust_widgets::json::marker_for_key(suffix) {\n\
             \x20           rust_widgets::json::bind_marker_event(id, suffix, marker, handler);\n\
             \x20       }\n\
             \x20       // An unrecognised suffix is impossible: the generator emits only keys from\n\
             \x20       // `is_wire_key`, and `node.props` is built by this file.\n\
             \x20   }\n\
             \x20   Some(id)\n\
             }\n",
        );
    }

    source
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_only_names_are_the_css_concepts_with_no_storage() {
        assert!(is_style_only_property("css_class"));
        assert!(is_style_only_property("transition"));
        // A real property is not style-only: dropping it would lose a value the user set.
        assert!(!is_style_only_property("width"));
        assert!(!is_style_only_property("text"));
    }

    #[test]
    fn wire_keys_are_not_properties() {
        assert!(is_wire_key("events"));
        assert!(is_wire_key("on_click"));
        assert!(!is_wire_key("text"));
    }

    #[test]
    fn the_generator_shares_the_runtime_wire_table() {
        assert!(
            shared_wire_rule_count() > 0,
            "the generator must consult the same rule table the runtime uses (T-3)"
        );
    }

    /// The generator's verdict must be the runtime's, not a look-alike.
    #[test]
    fn a_wire_verdict_comes_from_the_shared_table() {
        use crate::widget::capability::types::PropertyValueKind;
        use crate::widget::capability::{WireCompatibility, WireTarget as Target};

        // Text into a number is the rejection the rules single out, so it is the arm that would
        // differ first if the generator grew its own rules.
        assert!(matches!(
            wire_verdict_for(
                Some(PropertyValueKind::String),
                Target::Property(PropertyValueKind::Int)
            ),
            WireCompatibility::Rejected(_)
        ));
        // And a same-kind wire stays direct.
        assert_eq!(
            wire_verdict_for(
                Some(PropertyValueKind::Bool),
                Target::Property(PropertyValueKind::Bool)
            ),
            WireCompatibility::Direct
        );
    }

    #[test]
    fn the_two_targets_report_different_capacities() {
        assert_eq!(TargetProfile::Stripped.child_capacity(), MINI_CHILD_CAPACITY);
        assert!(TargetProfile::Default.child_capacity() > MINI_CHILD_CAPACITY);
    }

    #[test]
    fn availability_distinguishes_unknown_from_unavailable() {
        assert!(matches!(
            availability("definitely_not_a_control", TargetProfile::Default),
            Availability::Unknown
        ));
    }

    #[test]
    fn only_permitting_answers_allow_generation() {
        assert!(Availability::Local.permits_generation());
        assert!(Availability::CrossProfileCaveat.permits_generation());
        assert!(!Availability::Unavailable.permits_generation());
        assert!(!Availability::Unknown.permits_generation());
        assert!(Availability::CrossProfileCaveat.needs_attention());
        assert!(!Availability::Local.needs_attention());
    }

    #[test]
    fn a_report_is_clean_only_when_nothing_needs_attention() {
        let mut report = GenerationReport::default();
        assert!(report.is_clean());
        report.unsupported.push(GenerationGap {
            path: Vec::new(),
            widget: String::from("x"),
            reason: String::from("nope"),
        });
        assert!(!report.is_clean());
    }

    #[test]
    fn quoting_escapes_what_would_break_the_literal() {
        assert_eq!(quote("plain"), "\"plain\"");
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote("a\\b"), "\"a\\\\b\"");
        assert_eq!(quote("a\nb"), "\"a\\nb\"");
    }

    /// Every C0 control character must become a valid Rust escape.
    ///
    /// Pins the defect: only `" \\ \n \r \t` were escaped, so a control byte (e.g. `\u{7}`,
    /// the bell) in a document string was emitted raw into generated source, producing an invalid
    /// literal that would not compile. The fallback now writes `\u{..}` for the rest of C0 and DEL.
    #[test]
    fn quoting_escapes_the_whole_control_range() {
        assert_eq!(quote("\u{7}"), "\"\\u{7}\"");
        assert_eq!(quote("\u{1b}"), "\"\\u{1b}\"");
        assert_eq!(quote("\u{0}"), "\"\\u{0}\"");
        assert_eq!(quote("\u{7f}"), "\"\\u{7f}\"");
        // A printable non-ASCII character is left alone.
        assert_eq!(quote("中"), "\"中\"");
    }

    #[test]
    fn scalar_values_become_capability_values() {
        assert_eq!(capability_value_expr(&Value::Bool(true)), "CapabilityValue::Bool(true)");
        assert_eq!(
            capability_value_expr(&Value::String(String::from("hi"))),
            "CapabilityValue::String(String::from(\"hi\"))"
        );
        assert_eq!(capability_value_expr(&Value::Null), "CapabilityValue::Null");
    }

    #[test]
    fn a_non_scalar_does_not_become_a_misleading_string() {
        assert_eq!(capability_value_expr(&Value::Array(Vec::new())), "CapabilityValue::Null");
    }

    #[test]
    fn binding_names_are_derived_from_the_path() {
        assert_eq!(binding_name(&[]), "root");
        assert_eq!(binding_name(&[3]), "n_3");
        assert_eq!(binding_name(&[1, 2]), "n_1_2");
    }

    /// A declared wire must reach the generated **source**, on both templates.
    ///
    /// # The defect this pins
    ///
    /// `emit_node` carried a comment saying "Children, then the wires" and emitted none: the
    /// property loop skipped wire keys via `is_wire_key`, and nothing else ever consulted
    /// `declared_handlers`. A document declaring `events: { clicked: "on_save" }` therefore
    /// produced a program containing no reference to `on_save`, so the handler could never run —
    /// silently, because the name had been validated at generation time and looked fine.
    ///
    /// The existing mode-consistency test did not catch it either: it asserted the name was
    /// *publishable* (that `event_is_subscribable` accepts it), never that the generated file used
    /// it. This test asserts the generated source names the handler, which is the property that was
    /// actually broken.
    #[test]
    fn declared_wires_reach_the_generated_source() {
        const DOCUMENT: &str = r#"{
          "window": {
            "id": "root",
            "title": "Wires",
            "children": [
              {
                "button": {
                  "id": "save",
                  "text": "Save",
                  "events": { "clicked": "on_save" },
                  "on_change": "on_save_marker"
                }
              }
            ]
          }
        }"#;

        for target in [TargetProfile::Default, TargetProfile::Stripped] {
            let request = GenerationRequest {
                json: DOCUMENT.to_string(),
                target,
                width: 320,
                height: 240,
                function_name: String::from("build_wires"),
            };
            let generated = generate(&request).expect("the document must generate");
            for handler in ["on_save", "on_save_marker"] {
                assert!(
                    generated.source.contains(handler),
                    "{target:?}: the declared handler `{handler}` is absent from the generated \
                     source, so it could never run"
                );
            }
        }
    }

    /// On the default target the wire is **carried** as a prop and read back by `create_for`.
    ///
    /// Asserting the mechanism and not just the name: a future edit could keep the handler string
    /// in a comment and pass the test above while wiring nothing.
    #[test]
    fn the_default_target_carries_wires_as_prefixed_props() {
        const DOCUMENT: &str = r#"{
          "window": {
            "id": "root",
            "title": "Wires",
            "children": [
              { "button": { "id": "save", "text": "S", "events": { "clicked": "on_save" } } }
            ]
          }
        }"#;
        let request = GenerationRequest {
            json: DOCUMENT.to_string(),
            target: TargetProfile::Default,
            width: 320,
            height: 240,
            function_name: String::from("build_wires"),
        };
        let generated = generate(&request).expect("the document must generate");

        assert!(
            generated.source.contains(&format!("{WIRE_PROP_PREFIX}events.clicked")),
            "the wire must travel as a prefixed prop, not under its document key"
        );
        assert!(
            generated.source.contains("bind_published_event"),
            "the generated file must hand the wire to the library's own binder (rule #98)"
        );
        assert_eq!(generated.report.wires_emitted, 1, "one declared wire, one emitted");
    }

    /// A stripped target **refuses** a wire with a reason, because it has no widget registry to
    /// bind against.
    ///
    /// Refusing rather than dropping: the target cannot produce a working subscription, and a
    /// generated file that silently omitted the handler is the failure mode this whole fix is
    /// about.
    #[test]
    fn a_stripped_target_refuses_a_wire_rather_than_dropping_it() {
        const DOCUMENT: &str = r#"{
          "window": {
            "id": "root",
            "title": "Wires",
            "children": [
              { "button": { "id": "save", "text": "S", "events": { "clicked": "on_save" } } }
            ]
          }
        }"#;
        let request = GenerationRequest {
            json: DOCUMENT.to_string(),
            target: TargetProfile::Stripped,
            width: 320,
            height: 240,
            function_name: String::from("build_wires"),
        };
        let generated = generate(&request).expect("the document must generate");

        assert!(
            generated.report.unsupported.iter().any(|gap| gap.reason.contains("on_save")),
            "the refusal must name the handler so the reader can act on it: {:?}",
            generated.report.unsupported
        );
        assert!(!generated.report.is_clean(), "a refused wire is not a clean run");
    }
}
