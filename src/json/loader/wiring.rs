use super::*;

/// Wires one declared event binding to a live control — the single-call entry a **generated**
/// program uses (rule #98).
///
/// # Why this exists
///
/// The designer's generated program builds controls through its own `create_for` and never runs the
/// JSON loader — that is the point of generating code: the program links no parser. But the
/// *wiring* it must perform is exactly the loader's: validate the name against the control's
/// capability, choose the callback from the payload observation, and refuse a name whose trigger has
/// no signal. Rule #98 makes that the library's job, so a generator (or any host that learns its
/// wiring at run time) must not reimplement it — a second implementation is a second rule set that
/// agrees only until one side is edited.
///
/// Before this existed, the generator had **no** way to emit a wire at all: a document declaring
/// `events: { clicked: "on_save" }` produced a program containing no reference to `on_save`, so the
/// handler could never run — silently, because the name had been validated at generation time and
/// therefore looked fine. `examples/probe_generated_wires.rs` is the probe that measured it.
///
/// Returns whether the handler was bound. A marker whose trigger has no signal (an unroutable
/// `on_*` key) returns `false` and is reported at `warn!` and through the unwired-binding counter,
/// so a caller gets a fact rather than a binding that can never fire.
pub fn bind_event_binding(
    widget_id: crate::core::ObjectId,
    binding: crate::json::JsonEventBinding,
    handler_name: String,
) -> bool {
    JsonLoader::bind_one(widget_id, binding, handler_name)
}

/// The ASCII-lowercased spelling of a declared name, borrowing when it already is.
///
/// The loader compares declared widget/event names against lowercase literals. That used to
/// call `to_lowercase()` per comparison, which allocates a `String` even though the common
/// spelling in a document is already lowercase — the allocation bought nothing. Borrowing in
/// that case makes the comparison free; only a mixed-case declaration pays.
///
/// `to_ascii_lowercase` rather than `to_lowercase` is deliberate: these are control names,
/// every literal they are compared against is ASCII, and a non-ASCII name reaches the same
/// fallback arm either way (Unicode case folding could only turn it into a different
/// non-ASCII name).
pub(crate) fn lowered(name: &str) -> Cow<'_, str> {
    if name.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(name.to_ascii_lowercase())
    } else {
        Cow::Borrowed(name)
    }
}

/// The widget's kind in `Debug` spelling, for diagnostics and stylesheet matching.
///
/// A free function rather than a trait method: only this module's warnings and its stylesheet
/// pass need it, and the kind is already reachable through `Widget::kind`. It returns a
/// `'static` string rather than a formatted one — see `WidgetKind::css_name` for why.
pub(crate) fn kind_label(widget: &dyn Widget) -> &'static str {
    widget.kind().css_name()
}

/// Apply declarative stylesheet rules to a freshly created widget.
///
/// Four sources, applied in increasing precedence so a more specific source wins:
///
/// 1. The **active theme** resolved for the node's class (or kind). This is the
///    base appearance every control starts from; without it each control kept the
///    colours its constructor hardcoded and a theme switch had no effect.
/// 2. The **global stylesheet manager**'s registered sheets, in their own
///    priority order. This is the app-wide CSS layer.
/// 3. An inline `"css"` string on the node itself. Page-local rules beat
///    app-wide ones, which is what makes a one-off layout possible.
///
/// Matching uses the widget's kind plus the node's optional `"class"` and `"id"`,
/// so a rule written as `Button.primary` or `#ok_btn` selects the intended node.
/// The widget's kind is formatted in the `Debug` spelling the CSS selector parser
/// expects (`Button`, not `button`).
///
/// Each layer is merged *under* whatever the widget already has, so the explicit
/// JSON keys applied after this function always win. A malformed stylesheet is
/// reported and skipped: the widget is still created, because failing the whole
/// load over one bad rule would hide the rest of the layout.
pub(crate) fn apply_declared_styles(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
    ancestors: &[ElementRef<'_>],
) {
    let kind = kind_label(widget);
    let class = obj.get("class").and_then(|v| v.as_str());
    let id = obj.get("id").and_then(|v| v.as_str());
    let element = ElementRef { kind: widget.kind(), class, id, state: Some(widget.widget_state()) };

    // One clone, one write-back. This used to clone the widget's whole style and install it
    // once per layer — up to four deep copies per node, each briefly *applying* a non-final
    // style that observers (mirrors, a11y, the renderer's epoch) could see as a change. A
    // 500-node layout paid thousands of copies; and a node with no theme style and no
    // registered app-wide sheet paid two of them for no change at all. The layers below merge
    // into the same copy, which is installed once.
    let mut style = widget.style().clone();
    let mut touched = false;

    // 1. Active theme. The widget's **kind** determines its visual role; a node's
    //    `class` is only an override key, never a role name. Passing the class here used to
    //    replace the kind, so `<button class="primary">` was classified as `Surface` (because
    //    "primary" is not a control kind) and painted as a grey panel instead of a filled
    //    brand-coloured button — the class silently discarded the role it was meant to select.
    //
    //    This is the shared, revision-keyed form (G-401): a document names the same
    //    `(kind, class)` pair on many nodes, and the owned accessor used to take the theme
    //    mutex and rebuild the role's style for every one of them. The walk now pays one
    //    resolution per distinct pair per theme revision; every later node gets a refcount
    //    bump instead of a lock and a rebuild.
    if let Some(theme_style) = crate::theme::resolved_theme_style_for_shared(kind, class) {
        style.merge(&theme_style);
        touched = true;
    }

    // 2. App-wide sheets, respecting their registered priority. Skipped entirely when none are
    //    registered: an empty manager is the common case for a layout without app-wide CSS.
    if !crate::style::global_stylesheet_manager().is_empty() {
        let applied = {
            let manager = crate::style::global_stylesheet_manager();
            // STY-002: pass the control's live interaction state and the element chain built by
            // the loader. The production entry used to pass `None` for the state and no chain at
            // all, so every `:hover`/`:pressed`/`:focus`/`:disabled` rule and every
            // descendant/child combinator in an app-wide sheet was parsed, indexed and then could
            // never match the control it was written for.
            manager.apply_to_context(element, ancestors, &mut style)
        };
        if let Err(error) = applied {
            log::warn!(
                "JSON layout: a global stylesheet failed to apply to {kind} (class {class:?}, id \
                 {id:?}): {error}"
            );
        }
        // The style is installed even when a sheet reported an error: `apply_to` skips a
        // malformed sheet but still applies every valid one, so discarding the whole style
        // here would turn one bad app-wide sheet into "no CSS at all" for every widget.
        touched = true;
    }

    if touched {
        widget.set_style(style);
    }

    // 3. Page-local inline CSS on this node.
    if let Some(css) = obj.get("css").and_then(|v| v.as_str()) {
        match widget.apply_css_in(css, class, id, ancestors) {
            Ok(()) => {}
            Err(error) => log::warn!(
                "JSON layout: the inline \"css\" on {kind} (id {id:?}) could not be applied: \
                 {error}"
            ),
        }
    }
}

/// Apply common widget properties from a JSON object.
///
/// Two layers, in this order:
///
/// 1. **Name-driven** — every key that the control's own property contract
///    publishes is written through [`crate::json::properties`], so a property
///    added to any control is addressable from JSON with no change here. This is
///    what makes the loader cover every `WidgetKind` variant rather than a
///    hand-maintained subset — the kind is resolved from the capability schema, not
///    from a list kept in this file, so it cannot drift as controls are added.
/// 2. **Loader-owned** — keys that describe the style object as a whole (`padding`
///    / `margin` accept a number *or* a four-sided object), the geometry shorthand,
///    and the min/max size constraints, which have no single-property equivalent.
///
/// A key that neither layer recognises is reported as a warning naming the key and
/// the widget, so a typo (`"colour"` for `"color"`) is visible instead of loading
/// a silently default-styled control.
#[cfg(test)]
pub(crate) fn apply_properties(widget: &mut dyn Widget, obj: &serde_json::Map<String, Value>) {
    apply_properties_with_icon_resolver(widget, obj, None);
}

pub(crate) fn apply_properties_with_icon_resolver(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
    resolver: Option<&dyn crate::widget::icon_source::IconResourceResolver>,
) {
    let published =
        crate::widget::capability::properties_trait::widget_property_names(widget).unwrap_or(&[]);
    let conflicting_icon_declarations = obj.contains_key("icon_source")
        && obj.contains_key("icon_text")
        && published.contains(&"icon_source")
        && published.contains(&"icon_text");
    if conflicting_icon_declarations {
        log::warn!(
            "JSON layout: both legacy \"icon_text\" and typed \"icon_source\" were declared for \
             {}; neither icon value was applied",
            kind_label(widget)
        );
    }

    // ── Geometry ────────────────────────────────────────────
    // Applied first because the property layer also publishes `geometry`, and a
    // control's own setter is the authority; the shorthand below fills in the
    // common case of four separate keys.
    //
    // A **partial** set is honoured against the widget's existing rect rather than
    // ignored. Before this, `{"button": {"id": "b", "width": 200, "height": 40}}`
    // loaded with the size silently dropped: the shorthand only fired when all four
    // keys were present, and `is_loader_owned_key` kept the name-driven pass from
    // reporting them, so the author got a control at its default size and no warning —
    // the opposite of this module's documented contract that an unrecognised key is
    // named. Filling the missing components from the current geometry is the same
    // "start from what the control has" rule `apply_size_constraints` uses.
    apply_geometry_shorthand(widget, obj);

    // ── Style: padding / margin ─────────────────────────────
    // These two accept either a bare number (all sides) or an object with
    // top/right/bottom/left, which no single property name can express. They are
    // handled before the name-driven pass so the structured form is not rejected
    // as a type error by a property expecting a scalar.
    apply_style_padding(widget, obj, "padding", |widget, value| {
        let mut style = widget.style().clone();
        style.padding = value;
        widget.set_style(style);
    });
    if let Some(value) = obj.get("margin") {
        if let Some(p) = parse_spacing(value) {
            let margin = crate::style::Margin::new(p.top, p.right, p.bottom, p.left);
            let mut style = widget.style().clone();
            style.margin = margin;
            widget.set_style(style);
        }
    }

    // ── Name-driven pass ────────────────────────────────────
    // Iterate the JSON keys rather than a fixed list: that is what lets a control
    // whose properties this module has never seen be configured from JSON.
    for (key, value) in obj {
        if conflicting_icon_declarations && matches!(key.as_str(), "icon_text" | "icon_source") {
            continue;
        }
        // Resolve once per key. The loader-owned check and the apply below used to
        // resolve the same name and re-scan the same schema list twice per key.
        let published = crate::json::properties::resolve_property_name(widget, key);
        if is_loader_owned_key(key)
            && !published.is_some_and(|name| {
                crate::json::properties::resolved_schema_is_record_list(widget, name)
            })
        {
            continue;
        }
        let outcome = match published {
            Some(name) => {
                crate::json::properties::apply_resolved_property(widget, name, value, resolver)
            }
            None => ApplyOutcome::NotAProperty,
        };
        match outcome {
            ApplyOutcome::Applied => {}
            // The control does not publish this name. It may still be a
            // construction-time key the caller's own arm consumed (`items`, …),
            // so this is a warning rather than an error: the load still succeeds,
            // but the typo is visible.
            ApplyOutcome::NotAProperty => {
                log::warn!(
                    "JSON layout: {key:?} is not a property of this {} and no construction arm \
                     consumed it; the value ({value}) was ignored",
                    kind_label(widget)
                );
            }
            ApplyOutcome::Rejected => {
                log::warn!(
                    "JSON layout: property {key:?} of this {} rejected the value {value}; \
                     inspect the property's conversion, validation, or write diagnostics",
                    kind_label(widget)
                );
            }
        }
    }

    // ── Style: colors written as plain JSON strings ──────────
    // These are the documented JSON spellings. They are separate from the
    // name-driven pass because JSON carries a colour as a `#RRGGBB` string while
    // the property contract transports it as a value the control parses itself;
    // routing them here keeps the JSON surface stable for existing layouts.
    apply_hex_color(widget, obj, "background", |widget, color| {
        widget.set_background_color(Some(color))
    });
    apply_hex_color(widget, obj, "text_color", |widget, color| {
        widget.set_foreground_color(Some(color))
    });
    apply_hex_color(widget, obj, "border_color", |widget, color| {
        widget.set_border_color(Some(color))
    });

    // ── Style: border width / radius ────────────────────────
    if let Some(bw) = json_u32_at(obj, "border_width") {
        widget.set_border_width(bw);
    }
    if let Some(br) = json_u32_at(obj, "border_radius") {
        widget.set_border_radius(br);
    }

    // ── Size constraints ────────────────────────────────────
    apply_size_constraints(widget, obj);
}

/// Applies whichever of `x` / `y` / `width` / `height` the object supplies.
///
/// A missing component keeps the widget's current value, so the four keys are
/// independent: a document may set only a size, only an origin, or all four. Returns the
/// number of components applied, so the caller can tell "no geometry keys" from "all four".
pub(crate) fn apply_geometry_shorthand(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
) -> usize {
    let x = obj.get("x").and_then(|v| v.as_i64());
    let y = obj.get("y").and_then(|v| v.as_i64());
    let width = obj.get("width").and_then(|v| v.as_u64());
    let height = obj.get("height").and_then(|v| v.as_u64());
    let supplied = [x.is_some(), y.is_some(), width.is_some(), height.is_some()];
    if !supplied.iter().any(|&present| present) {
        return 0;
    }

    let current = widget.geometry();
    // `width`/`height` are `u64` but `Rect::from_i64` takes `i64`: a bare `w as i64` wrapped a
    // value above `i64::MAX` to a *negative* extent, which `from_i64` then clamped to `0`, so a
    // document asking for an enormous width laid out as a zero-width control. Clamping keeps
    // the magnitude, and `from_i64` clamps toward the screen's own limits from there.
    let extent = |raw: u64| i64::try_from(raw).unwrap_or(i64::MAX);
    let rect = Rect::from_i64(
        x.unwrap_or(current.x as i64),
        y.unwrap_or(current.y as i64),
        width.map(extent).unwrap_or(current.width as i64),
        height.map(extent).unwrap_or(current.height as i64),
    );
    widget.set_geometry(rect);
    supplied.iter().filter(|&&present| present).count()
}

/// Keys this module consumes structurally, so the name-driven pass must not also
/// try to resolve them as properties.
///
/// # Why the list is exhaustive and tested
///
/// A key appearing here is a claim that the loader has its own (possibly richer)
/// handling for it. The converse matters just as much: a key a `create_widget` arm
/// reads but that is *absent* from this list makes the name-driven pass report
/// ``was ignored'' about a value the arm already applied — a wrong diagnostic that
/// sends an author chasing a non-bug. Six keys were in exactly that state
/// (`tristate`, `password`, `word_wrap`, `tab_shape`, `h_policy`, `v_policy`,
/// `alpha`), because the arm-level reads and this list are maintained separately.
///
/// `every_arm_consumed_key_is_loader_owned` in the tests extracts the arm keys from
/// this file and asserts each one is listed here, so the two cannot drift apart
/// again.
///
/// # Why this is `pub`
///
/// The designer's **generator** has to answer the same question and must not grow a
/// second copy of the answer (rule #101: one concept, one implementation path). It
/// used to grow one: [`crate::designer::is_style_only_property`] listed only CSS
/// *internal* names (`css_class`, `selector`, `transition`, …), none of which is a
/// JSON key, so every key below was emitted as a `.prop(..)` write on the generated
/// `Node` and then refused by the control's property router — `background`,
/// `padding`, `x`, `width`, … all reported `UnknownProperty` in a generated program's
/// `ApplyReport`, while mode 1 applied the same document correctly.
pub fn is_loader_owned_key(key: &str) -> bool {
    matches!(
        key,
        // Geometry shorthand (four keys form one geometry).
        "x" | "y" | "width" | "height"
        // Structured spacing (a number *or* a four-sided object).
        | "padding" | "margin"
        // Colours in JSON `#RRGGBB` form; applied through the dedicated hex path.
        | "background" | "text_color" | "border_color"
        | "border_width" | "border_radius"
        // Size constraints read as a group.
        | "min_width" | "min_height" | "max_width" | "max_height"
        // Identity and wiring, consumed by `instantiate_node`.
        | "id" | "text" | "title" | "class" | "css" | "tooltip"
        // Construction-time keys owned by a `create_widget` arm or the layout
        // machinery: arrays and sub-objects that describe the widget's content
        // rather than a scalar state property.
        | "children" | "layout" | "items" | "stretch"
        // Container content routing (issue-20261104 DM-06/07/08/11/12). These name the
        // side table a container keeps its content in — pages of a stack or wizard,
        // sections of an accordion, panes of a splitter, docks of a dock host, slots of
        // an adaptive scaffold, and the menu/tool-bar entry arrays. `route_container_content`
        // consumes them through the controls' own methods; none is a scalar property, so
        // the name-driven pass must not report them as ignored.
        | "pages" | "sections" | "panes" | "menus" | "actions" | "slot"
        // Per-node breakpoint visibility (RW-117), consumed by `instantiate_node` before the
        // control is created. It is structure rather than a property of any control.
        | "breakpoints"
        // Per-child cell placement, consumed by `apply_child_placement` (which forwards
        // them to a layout that has cells). They belong here rather than being left out:
        // they name no scalar property on any control, so the name-driven pass would
        // otherwise warn that a value the loader just applied "was ignored".
        | "col" | "row" | "col_span" | "row_span"
        // Per-child region placement for `border` layouts, consumed by
        // `apply_child_placement` through `place_widget_in_region_layout`.
        | "region" | "extent"
        // Per-child absolute placement (`anchor`) and constraint relationships (`constraints`),
        // consumed by `apply_child_placement` (API2-01).
        | "anchor" | "constraints"
        // Construction-time scalar keys a `create_widget` arm reads directly, under
        // a spelling that is deliberately *not* the control's property name (an arm
        // may need to translate a JSON word into an enum, or set two fields from
        // one key). Each one is applied by its arm, so the name-driven pass must
        // leave it alone rather than warn about it:
        //   tristate  → checkbox.set_tristate_enabled
        //   password  → lineedit.set_echo_mode
        //   word_wrap → textedit.set_line_wrap
        //   tab_shape → tabwidget.set_tab_shape
        //   h_policy / v_policy → scrollarea.set_*_scroll_bar_policy
        //   alpha     → colordialog.set_options_alpha
        | "tristate" | "password" | "word_wrap" | "tab_shape"
        | "h_policy" | "v_policy" | "alpha"
        // Events, wired after registration. The `events` object holds published names (rule
        // #101's merged route) and the `on_*` keys are the compatibility spellings. The key list
        // is read from `event_route::MARKER_KEYS` rather than restated, so a key the wiring reads
        // can never be missing here and be reported as an unknown property.
        | "events"
    ) || crate::json::is_marker_key(key)
}

/// Whether a node's `breakpoints` list includes the tier currently in force (RW-117).
///
/// Absent list = every tier. A malformed list is reported and treated as absent (removing a node
/// because its tier spelling was mistyped would be a silent edit). An unknown tier token is named
/// and ignored. A root excluded by its own list is an error: a document with no root is not a
/// preview, it is a contradiction.
pub(crate) fn breakpoints_include(
    obj: &serde_json::Map<String, Value>,
    widget_type: &str,
    is_root: bool,
) -> Result<bool, String> {
    let Some(value) = obj.get("breakpoints") else {
        return Ok(true);
    };
    let Some(tiers) = value.as_array() else {
        log::warn!(
            "JSON layout: `breakpoints` on '{widget_type}' must be an array of tier names \
             (compact, medium, expanded); the node was kept in every tier"
        );
        return Ok(true);
    };
    let current = current_breakpoint_token();
    let mut included = false;
    for entry in tiers {
        let Some(token) = entry.as_str() else {
            log::warn!(
                "JSON layout: a `breakpoints` entry on '{widget_type}' is not a string; it was \
                 ignored"
            );
            continue;
        };
        match token {
            "compact" | "medium" | "expanded" => {
                if token == current {
                    included = true;
                }
            }
            other => log::warn!(
                "JSON layout: `{other}` on '{widget_type}' is not a breakpoint tier (expected \
                 compact, medium or expanded); it was ignored"
            ),
        }
    }
    if !included && is_root {
        return Err(format!(
            "the root '{widget_type}' is excluded by its `breakpoints` list; the tier in force is \
             '{current}'"
        ));
    }
    Ok(included)
}

/// The tier token in force for the current load.
///
/// Under the declarative layer this is the same thread-local fact `Node::breakpoint` reads. A
/// build without that layer has no viewport channel, so the honest answer is the widest tier —
/// the same default `Breakpoint::current` uses.
#[cfg(declarative_view)]
pub(crate) fn current_breakpoint_token() -> &'static str {
    crate::view::Breakpoint::current().token()
}

#[cfg(not(declarative_view))]
pub(crate) fn current_breakpoint_token() -> &'static str {
    "expanded"
}

/// Apply a `#RRGGBB`-style colour key when it is present and parses.
///
/// A colour that does not parse is reported: silently keeping the previous colour
/// is how a layout ends up looking almost right with no indication why.
pub(crate) fn apply_hex_color(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
    key: &str,
    apply: fn(&mut dyn Widget, Color),
) {
    let Some(raw) = obj.get(key).and_then(|v| v.as_str()) else {
        return;
    };
    match Color::parse_hex(raw) {
        Some(color) => apply(widget, color),
        None => log::warn!(
            "JSON layout: colour {raw:?} for {key:?} on this {} is not a #RGB/#RRGGBB/\
             #RRGGBBAA literal and was ignored",
            kind_label(widget)
        ),
    }
}

pub(crate) fn json_geometry(obj: &serde_json::Map<String, Value>) -> Rect {
    Rect::from_i64(
        obj.get("x").and_then(|value| value.as_i64()).unwrap_or(0),
        obj.get("y").and_then(|value| value.as_i64()).unwrap_or(0),
        obj.get("width").and_then(|value| value.as_i64()).unwrap_or(100),
        obj.get("height").and_then(|value| value.as_i64()).unwrap_or(100),
    )
}

/// Reads a document's `"id"` as an `ObjectId`.
///
/// A name that is not a number is hashed into a stable non-zero id rather than falling back
/// to a shared sentinel: two nodes with different names must not resolve to one id, or the
/// second registration would overwrite the first one's binding.
pub(crate) fn parse_declared_id(id_str: &str) -> ObjectId {
    if let Ok(numeric) = id_str.parse::<ObjectId>() {
        return numeric;
    }
    let mut hash: ObjectId = 0xcbf2_9ce4_8422_2325;
    for byte in id_str.as_bytes() {
        hash ^= ObjectId::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash | 1
}

/// Hands a created widget to the runtime and returns the id it is mounted under.
///
/// # Why the mount goes through the backend
///
/// `crate::control_backend::get_control_backend().mount_widget_box(..)` is the one funnel every
/// created control passes through: it assigns and registers the id, applies the active theme,
/// records the parent/child relation in both directions, and gives a window its host surface.
/// A second implementation of that here would drift from it — and this loader's previous
/// behaviour, which mounted nothing at all, is what that drift looks like when it has run its
/// course.
///
/// # Why a failure is reported
///
/// On a thread with no widget runtime, or with a host that refuses the creation, there is no
/// live control to address. The document still loads (the binding is intact and a caller can
/// read its structure), but every handle resolved from it would silently answer with defaults
/// and every handler it declared would be dropped. Saying so is what makes that queryable
/// rather than mysterious.
pub(crate) fn mount_declared_widget(
    widget: Box<dyn Widget>,
    parent_id: Option<ObjectId>,
    declared_id: ObjectId,
    widget_type: &str,
) -> ObjectId {
    let backend = crate::control_backend::get_control_backend();
    let id = backend.mount_widget_box(widget, parent_id, declared_id);
    if id == 0 {
        log::warn!(
            "JSON layout: the '{widget_type}' node could not be mounted (no live widget runtime \
             on this thread, or its parent was refused); handles resolved from it will report \
             defaults and its declared handlers will not fire"
        );
    }
    id
}

/// Reads a numeric range from JSON, preferring the names the control publishes.
///
/// `slider`, `scrollbar`, `progressbar` and `spinbox` publish
/// `minimum`/`maximum` (see their `PropertySchema` rows), so those are the
/// documented spellings. Their arms used to read `min`/`max` only — names none of
/// those four controls publishes — which produced two problems:
///
/// * `min`/`max` were not resolvable by the name-driven pass either, so an author
///   using them got a *"no construction arm consumed it; the value was ignored"*
///   warning about a value the arm had just applied;
/// * `min`/`max` **do** name a real property on `cupertino_slider`, where they are
///   `Float`. One spelling therefore meant an integer range on four controls and a
///   float pair on a fifth, with no documented reason.
///
/// Both spellings are accepted here, with the published name winning, so existing
/// layouts keep working while the canonical form is what the schema advertises.
pub(crate) fn read_json_range(obj: &serde_json::Map<String, Value>) -> (Option<i64>, Option<i64>) {
    let read = |published: &str, alias: &str| {
        obj.get(published).or_else(|| obj.get(alias)).and_then(|value| value.as_i64())
    };
    (read("minimum", "min"), read("maximum", "max"))
}

/// The numeric-range surface `slider`, `scrollbar` and `progressbar` share.
///
/// The three widgets take the same `minimum`/`maximum`/`value` triple from the same JSON
/// spellings, and the loader used to carry three copies of that parse — three places to change
/// and two to forget. The trait only *reaches* each widget's own setters; clamping,
/// re-clamping and signal behaviour stay where they are implemented.
pub(crate) trait RangeLike {
    fn set_range(&mut self, minimum: i32, maximum: i32);
    fn set_maximum(&mut self, maximum: i32);
    fn set_value(&mut self, value: i32);
}

impl RangeLike for Slider {
    fn set_range(&mut self, minimum: i32, maximum: i32) {
        Slider::set_range(self, minimum, maximum);
    }

    fn set_maximum(&mut self, maximum: i32) {
        Slider::set_maximum(self, maximum);
    }

    fn set_value(&mut self, value: i32) {
        Slider::set_value(self, value);
    }
}

impl RangeLike for ScrollBar {
    fn set_range(&mut self, minimum: i32, maximum: i32) {
        ScrollBar::set_range(self, minimum, maximum);
    }

    fn set_maximum(&mut self, maximum: i32) {
        ScrollBar::set_maximum(self, maximum);
    }

    fn set_value(&mut self, value: i32) {
        ScrollBar::set_value(self, value);
    }
}

impl RangeLike for ProgressBar {
    fn set_range(&mut self, minimum: i32, maximum: i32) {
        ProgressBar::set_range(self, minimum, maximum);
    }

    fn set_maximum(&mut self, maximum: i32) {
        ProgressBar::set_maximum(self, maximum);
    }

    fn set_value(&mut self, value: i32) {
        ProgressBar::set_value(self, value);
    }
}

/// Applies the `minimum`/`maximum`/`value` triple a range-like widget declares.
///
/// Two rules are shared by all three widgets and were spelled out identically in each arm:
///
/// * a declared `minimum` without a `maximum` pairs with 100, the maximum every one of them is
///   constructed with, so `{"minimum": 5}` means `5..=100` rather than `5..=i32::MAX`;
/// * a `maximum` on its own widens the existing range (`set_maximum`) instead of also resetting
///   the lower bound, which keeps a document that states one end from silently moving the other.
///
/// `spinbox` deliberately does not use this: it applies `minimum` and `maximum` independently
/// rather than as a range, so folding it in would change what it does.
pub(crate) fn apply_range_like<W: RangeLike>(obj: &serde_json::Map<String, Value>, widget: &mut W) {
    let (minimum, maximum) = read_json_range(obj);
    if let Some(minimum) = minimum {
        widget.set_range(i32_from_i64(minimum), i32_from_i64(maximum.unwrap_or(100)));
    } else if let Some(maximum) = maximum {
        widget.set_maximum(i32_from_i64(maximum));
    }
    if let Some(value) = obj.get("value").and_then(|value| value.as_i64()) {
        widget.set_value(i32_from_i64(value));
    }
}

/// Reads an `orientation` string as these widgets spell it.
///
/// `None` means the document named no orientation, or named one nobody defines. Both leave the
/// control on the orientation it was constructed with, which is what `slider` and `scrollbar`
/// already did for an unknown value. `progressbar` used to force `Horizontal` for an unknown
/// string; that is the orientation it is constructed with, so the resulting state is unchanged.
///
/// Test-only since BIND-05: the construction arms no longer apply `orientation` themselves — the
/// name-driven property pass does — so the only callers left are the tests that pin the shared
/// range contract.
#[cfg(test)]
pub(crate) fn json_orientation(
    obj: &serde_json::Map<String, Value>,
) -> Option<crate::core::Orientation> {
    use crate::core::Orientation;
    match obj.get("orientation").and_then(|value| value.as_str())? {
        "horizontal" => Some(Orientation::Horizontal),
        "vertical" => Some(Orientation::Vertical),
        _ => None,
    }
}

/// Calls `add` with every string in the document's `items` array.
///
/// A non-string entry is skipped rather than stringified: an item is a caption, and a `null` or
/// a number in the array is a document mistake, not a label to invent.
pub(crate) fn for_each_json_item(
    obj: &serde_json::Map<String, Value>,
    mut add: impl FnMut(String),
) {
    if let Some(items) = obj.get("items").and_then(|value| value.as_array()) {
        for item in items {
            if let Some(text) = item.as_str() {
                add(text.to_string());
            }
        }
    }
}

/// Reads a JSON `u64` field and narrows it to `u32`, clamping at the bounds.
///
/// A JSON number is an arbitrary `u64` (or a float), so `as u32` is a truncation rather
/// than a conversion: `4294967296` wrapped to `0`, and a spacing/size the author wrote
/// large came out as none. Clamping keeps the direction and magnitude the document stated.
pub(crate) fn json_u32_at(obj: &serde_json::Map<String, Value>, key: &str) -> Option<u32> {
    obj.get(key).and_then(|v| v.as_u64()).map(|raw| u32::try_from(raw).unwrap_or(u32::MAX))
}

/// Reads a JSON `u64` field and narrows it to `usize`, clamping at the bounds.
pub(crate) fn json_usize_at(obj: &serde_json::Map<String, Value>, key: &str) -> Option<usize> {
    obj.get(key).and_then(|v| v.as_u64()).map(|raw| usize::try_from(raw).unwrap_or(usize::MAX))
}

/// Narrows an `i64` (typically a JSON range bound) to `i32`, clamping at the bounds.
///
/// The JSON range reader yields `i64`; a bare `as i32` wrapped an out-of-range bound to the
/// opposite sign, so a slider asking for `2147483648` arrived as `i32::MIN`. Clamping keeps
/// the direction of the overflow.
pub(crate) fn i32_from_i64(raw: i64) -> i32 {
    i32::try_from(raw).unwrap_or(if raw > 0 { i32::MAX } else { i32::MIN })
}

/// Apply min/max size constraints from JSON object.
pub(crate) fn apply_size_constraints(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
) {
    let min_w = obj.get("min_width").and_then(|v| v.as_u64());
    let min_h = obj.get("min_height").and_then(|v| v.as_u64());
    let max_w = obj.get("max_width").and_then(|v| v.as_u64());
    let max_h = obj.get("max_height").and_then(|v| v.as_u64());

    // `as u32` on a `u64` wrapped an over-large constraint to a tiny one (`min_width` of
    // `4294967296` became `0`). Clamping keeps the constraint the document stated.
    let clamp_u32 = |raw: u64| u32::try_from(raw).unwrap_or(u32::MAX);

    if min_w.is_some() || min_h.is_some() {
        let current = widget.min_size().unwrap_or(crate::core::Size::new(0, 0));
        widget.set_min_size(Some(crate::core::Size::new(
            min_w.map(clamp_u32).unwrap_or(current.width),
            min_h.map(clamp_u32).unwrap_or(current.height),
        )));
    }
    if max_w.is_some() || max_h.is_some() {
        let current = widget.max_size().unwrap_or(crate::core::Size::new(u32::MAX, u32::MAX));
        widget.set_max_size(Some(crate::core::Size::new(
            max_w.map(clamp_u32).unwrap_or(current.width),
            max_h.map(clamp_u32).unwrap_or(current.height),
        )));
    }
}

/// Parse a padding/margin value from JSON: either a single number
/// (applied to all sides) or an object with top/right/bottom/left keys.
pub(crate) fn parse_spacing(value: &Value) -> Option<crate::style::Padding> {
    match value {
        Value::Number(n) => {
            let v = n.as_u64()?.try_into().ok()?;
            Some(crate::style::Padding::all(v))
        }
        Value::Object(map) => {
            let top = json_u32_at(map, "top").unwrap_or(0);
            let right = json_u32_at(map, "right").unwrap_or(0);
            let bottom = json_u32_at(map, "bottom").unwrap_or(0);
            let left = json_u32_at(map, "left").unwrap_or(0);
            if top == 0 && right == 0 && bottom == 0 && left == 0 {
                None
            } else {
                Some(crate::style::Padding::new(top, right, bottom, left))
            }
        }
        _ => None,
    }
}

/// Apply a padding property to a widget.
pub(crate) fn apply_style_padding(
    widget: &mut dyn Widget,
    obj: &serde_json::Map<String, Value>,
    key: &str,
    apply: fn(&mut dyn Widget, crate::style::Padding),
) {
    if let Some(value) = obj.get(key) {
        if let Some(padding) = parse_spacing(value) {
            apply(widget, padding);
        }
    }
}

/// The `on_*` handler keys a JSON node may declare, with the marker each one means.
///
/// # Why the tuple-returning extractors were deleted (BLUE19 T-8)
///
/// This used to be two functions returning `(Option<String>, ...)` tuples that the loader
/// destructured positionally: `on_click`/`on_change` from one, and six more from the other. Two
/// problems, both of which the table below removes:
///
/// 1. **Positional coupling.** A key added to `extract_extended_event_handlers` but not to its
///    return type — or read in a different order at the call site — silently wired a handler to
///    the wrong trigger. Nothing could detect it, because the tuple has no names.
/// 2. **Two sources of truth.** The loader also listed all eight keys by hand in its
///    "not a property" pattern, so the extraction set and the key set could disagree.
///
/// The list now lives once, in [`crate::json::MARKER_KEYS`], and is read by name.
///
/// The re-export is `#[cfg(test)]` because production code reaches the table through
/// `crate::json::MARKER_KEYS` directly; only this module's tests need it in scope.
#[cfg(test)]
pub use crate::json::event_route::MARKER_KEYS;

/// The name→kind table, retained for tests only.
///
/// # Why it is no longer used in production
///
/// Registration reads the **live control's** `kind()` instead (see
/// `instantiate_node`). This table is a hand-written subset of the names the factory
/// routes, and its `_ => WidgetKind::Button` fallback meant any name it did not list
/// — `icon`, `chip`, `table`, `progress_bar`, … — was recorded as a `Button`, so the
/// widget registry disagreed with the tree the loader had just built.
///
/// It is kept under `#[cfg(test)]` because two tests assert properties *of the old
/// table* to demonstrate the fix (that a control's real kind differs from this
/// fallback). Deleting it would delete the evidence that the defect existed.
#[cfg(test)]
pub(crate) fn infer_kind(widget_type: &str) -> WidgetKind {
    match widget_type.to_lowercase().as_str() {
        // Non-gated variants (available in all profiles)
        "arc" => WidgetKind::Arc,
        "button" => WidgetKind::Button,
        "checkbox" => WidgetKind::CheckBox,
        "combobox" => WidgetKind::ComboBox,
        "dropdown" => WidgetKind::Dropdown,
        "frame" => WidgetKind::Frame,
        "groupbox" => WidgetKind::GroupBox,
        "imageview" => WidgetKind::ImageView,
        "keyboard" => WidgetKind::Keyboard,
        "label" => WidgetKind::Label,
        "line" => WidgetKind::Line,
        "lineedit" => WidgetKind::LineEdit,
        "listbox" => WidgetKind::ListBox,
        "meter" => WidgetKind::Meter,
        "minicanvas" => WidgetKind::MiniCanvas,
        "minichart" => WidgetKind::MiniChart,
        #[cfg(not(alloc_frugal))]
        "colorpicker" => WidgetKind::ColorPicker,
        #[cfg(not(alloc_frugal))]
        "toast" => WidgetKind::Toast,
        #[cfg(not(alloc_frugal))]
        "splashscreen" => WidgetKind::SplashScreen,
        "candlestick_chart" => WidgetKind::CandlestickChart,
        "candlestick" => WidgetKind::CandlestickChart,
        "kline" => WidgetKind::CandlestickChart,
        "k_line" => WidgetKind::CandlestickChart,
        "k_line_chart" => WidgetKind::CandlestickChart,
        "ohlc_chart" => WidgetKind::CandlestickChart,
        "volume_chart" => WidgetKind::VolumeChart,
        "volume" => WidgetKind::VolumeChart,
        "volume_histogram" => WidgetKind::VolumeChart,
        "volume_bars" => WidgetKind::VolumeChart,
        "depth_chart" => WidgetKind::DepthChart,
        "market_depth" => WidgetKind::DepthChart,
        "depth_graph" => WidgetKind::DepthChart,
        "liquidity_chart" => WidgetKind::DepthChart,
        "order_book" => WidgetKind::OrderBook,
        "orderbook" => WidgetKind::OrderBook,
        "book_ladder" => WidgetKind::OrderBook,
        "market_depth_ladder" => WidgetKind::OrderBook,
        "quote_board" => WidgetKind::QuoteBoard,
        "quotes" => WidgetKind::QuoteBoard,
        "watchlist" => WidgetKind::QuoteBoard,
        "quote_table" => WidgetKind::QuoteBoard,
        "market_watch" => WidgetKind::QuoteBoard,
        "indicator_chart" => WidgetKind::IndicatorChart,
        "indicator" => WidgetKind::IndicatorChart,
        "technical_indicator" => WidgetKind::IndicatorChart,
        "oscillator" => WidgetKind::IndicatorChart,
        "macd_chart" => WidgetKind::IndicatorChart,
        "panel" => WidgetKind::Panel,
        "progressbar" => WidgetKind::ProgressBar,
        "radiobutton" => WidgetKind::RadioButton,
        "roller" => WidgetKind::Roller,
        "scrollarea" => WidgetKind::ScrollArea,
        "scrollbar" => WidgetKind::ScrollBar,
        "slider" => WidgetKind::Slider,
        "spinner" => WidgetKind::Spinner,
        "spinbox" => WidgetKind::SpinBox,
        "switch" => WidgetKind::Switch,
        "textarea" => WidgetKind::TextArea,
        "window" => WidgetKind::Window,
        // cfg(not(alloc_frugal)) variants
        #[cfg(not(alloc_frugal))]
        "autocompleteedit" => WidgetKind::AutoCompleteEdit,
        #[cfg(not(alloc_frugal))]
        "barchart" => WidgetKind::BarChart,
        #[cfg(not(alloc_frugal))]
        "calendar" => WidgetKind::Calendar,
        #[cfg(not(alloc_frugal))]
        "canvas" => WidgetKind::Canvas,
        #[cfg(not(alloc_frugal))]
        "chart" => WidgetKind::Chart,
        #[cfg(not(alloc_frugal))]
        "colordialog" => WidgetKind::ColorDialog,
        #[cfg(not(alloc_frugal))]
        "contextmenu" => WidgetKind::ContextMenu,
        #[cfg(not(alloc_frugal))]
        "dialog" => WidgetKind::Dialog,
        #[cfg(not(alloc_frugal))]
        "dockpanel" => WidgetKind::DockPanel,
        #[cfg(not(alloc_frugal))]
        "dropdownmenu" => WidgetKind::DropdownMenu,
        #[cfg(not(alloc_frugal))]
        "filedialog" => WidgetKind::FileDialog,
        #[cfg(not(alloc_frugal))]
        "floatinglabel" => WidgetKind::FloatingLabel,
        #[cfg(not(alloc_frugal))]
        "fontdialog" => WidgetKind::FontDialog,
        #[cfg(not(alloc_frugal))]
        "grid" => WidgetKind::Grid,
        #[cfg(not(alloc_frugal))]
        "icon" => WidgetKind::Icon,
        #[cfg(not(alloc_frugal))]
        "inputdialog" => WidgetKind::InputDialog,
        #[cfg(not(alloc_frugal))]
        "linechart" => WidgetKind::LineChart,
        #[cfg(not(alloc_frugal))]
        "listview" => WidgetKind::ListView,
        #[cfg(not(alloc_frugal))]
        "maskededit" => WidgetKind::MaskedEdit,
        #[cfg(not(alloc_frugal))]
        "mdiarea" => WidgetKind::MdiArea,
        #[cfg(not(alloc_frugal))]
        "menu" => WidgetKind::Menu,
        #[cfg(not(alloc_frugal))]
        "menubar" => WidgetKind::MenuBar,
        #[cfg(not(alloc_frugal))]
        "menubutton" => WidgetKind::MenuButton,
        #[cfg(not(alloc_frugal))]
        "menuitem" => WidgetKind::MenuItem,
        #[cfg(not(alloc_frugal))]
        "messagebox" => WidgetKind::MessageBox,
        #[cfg(not(alloc_frugal))]
        "multiselectcombobox" => WidgetKind::MultiSelectComboBox,
        #[cfg(not(alloc_frugal))]
        "piechart" => WidgetKind::PieChart,
        #[cfg(not(alloc_frugal))]
        "popover" => WidgetKind::Popover,
        #[cfg(not(alloc_frugal))]
        "popupwindow" => WidgetKind::PopupWindow,
        #[cfg(not(alloc_frugal))]
        "progresscircle" => WidgetKind::ProgressCircle,
        #[cfg(not(alloc_frugal))]
        "rangeslider" => WidgetKind::RangeSlider,
        #[cfg(not(alloc_frugal))]
        "rating" => WidgetKind::Rating,
        #[cfg(not(alloc_frugal))]
        "refreshcontrol" => WidgetKind::RefreshControl,
        #[cfg(not(alloc_frugal))]
        "richedit" => WidgetKind::RichEdit,
        #[cfg(not(alloc_frugal))]
        "searchbar" => WidgetKind::SearchBar,
        #[cfg(not(alloc_frugal))]
        "segmentedbutton" => WidgetKind::SegmentedButton,
        #[cfg(not(alloc_frugal))]
        "sparkline" => WidgetKind::Sparkline,
        #[cfg(not(alloc_frugal))]
        "splitter" => WidgetKind::Splitter,
        #[cfg(not(alloc_frugal))]
        "statusbar" => WidgetKind::StatusBar,
        #[cfg(not(alloc_frugal))]
        "stepper" => WidgetKind::Stepper,
        #[cfg(not(alloc_frugal))]
        "tabbar" => WidgetKind::TabBar,
        #[cfg(not(alloc_frugal))]
        "table" => WidgetKind::Table,
        #[cfg(not(alloc_frugal))]
        "tabwidget" => WidgetKind::TabWidget,
        #[cfg(not(alloc_frugal))]
        "textedit" => WidgetKind::TextEdit,
        #[cfg(not(alloc_frugal))]
        "togglebutton" => WidgetKind::ToggleButton,
        #[cfg(not(alloc_frugal))]
        "toolbar" => WidgetKind::ToolBar,
        #[cfg(not(alloc_frugal))]
        "tooltip" => WidgetKind::Tooltip,
        #[cfg(not(alloc_frugal))]
        "treeview" => WidgetKind::TreeView,
        _ => WidgetKind::Button,
    }
}

/// Parse a JSON layout string into a widget tree.
///
/// This is a convenience wrapper around [`JsonLoader::load`].
pub fn load_layout_from_str(json_str: &str) -> Result<BoundJsonLayout, String> {
    JsonLoader::load(json_str)
}
