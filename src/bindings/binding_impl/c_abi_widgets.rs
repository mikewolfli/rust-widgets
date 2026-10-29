// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! C ABI widget operations: scrolling, list items, layout, and style.
use super::*;
use crate::c_try;
use core::ffi::c_char;
use core::ffi::c_int;
use core::ffi::c_uint;

/// Destination codes for [`rw_widget_scroll_to`].
///
/// These are a stable ABI: the numeric values are part of the contract, so a new
/// destination must be appended rather than inserted.
pub const RW_SCROLL_TO_TOP: c_int = 0;

/// See [`RW_SCROLL_TO_TOP`].
pub const RW_SCROLL_TO_BOTTOM: c_int = 1;

/// See [`RW_SCROLL_TO_TOP`].
pub const RW_SCROLL_TO_LEFT: c_int = 2;

/// See [`RW_SCROLL_TO_TOP`].
pub const RW_SCROLL_TO_RIGHT: c_int = 3;

#[no_mangle]
/// Sets the scroll offset of a scrollable control.
///
/// The offset is clamped to the content extent, so an out-of-range request is not
/// an error: it scrolls as far as the content allows. Returns `false` when
/// `widget_id` does not address a control that scrolls (an unknown id, or a
/// control with no scroll offset) — a real "no" rather than a silent success.
///
/// Currently `scroll_area` answers; read the resulting offset back with
/// [`rw_get_widget_property`] where the control publishes `scroll_x` / `scroll_y`.
///
/// # Why this is a downcast and not a property write
///
/// A scroll offset is a consequence of the content and the viewport, not a free
/// value: `set_scroll_position` clamps it and emits `scroll_position_changed`.
/// Routing it through the property layer would either bypass that clamping or
/// duplicate it, so the control's own method is the single implementation.
///
pub extern "C" fn rw_widget_set_scroll_position(widget_id: u64, x: c_int, y: c_int) -> CBool {
    c_try!({
        let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            match crate::widget::capability::coercion::widget_as_mut::<crate::widget::ScrollArea>(
                widget,
            ) {
                Some(area) => {
                    area.set_scroll_position(x, y);
                    true
                }
                None => false,
            }
        })
        .unwrap_or(false);
        if applied {
            crate::widget::runtime::request_repaint(widget_id);
        }
        applied
    })
}

#[no_mangle]
/// Scrolls a control to one of the four edges.
///
/// `where_` is one of [`RW_SCROLL_TO_TOP`] / [`RW_SCROLL_TO_BOTTOM`] /
/// [`RW_SCROLL_TO_LEFT`] / [`RW_SCROLL_TO_RIGHT`]; any other value is rejected
/// (returns `false`) rather than quietly defaulting to an edge.
///
pub extern "C" fn rw_widget_scroll_to(widget_id: u64, where_: c_int) -> CBool {
    c_try!({
        let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            let Some(area) = crate::widget::capability::coercion::widget_as_mut::<
                crate::widget::ScrollArea,
            >(widget) else {
                return false;
            };
            match where_ {
                RW_SCROLL_TO_TOP => area.scroll_to_top(),
                RW_SCROLL_TO_BOTTOM => area.scroll_to_bottom(),
                RW_SCROLL_TO_LEFT => area.scroll_to_left(),
                RW_SCROLL_TO_RIGHT => area.scroll_to_right(),
                _ => return false,
            }
            true
        })
        .unwrap_or(false);
        if applied {
            crate::widget::runtime::request_repaint(widget_id);
        }
        applied
    })
}

#[no_mangle]
/// Adds one item to a list-like control.
///
/// Works for every control that holds strings through `append_widget_list_item`
/// (`list_box`, `combo_box`), which is what keeps this one entry point instead of
/// one per control. Returns the new item count, or `0` when the control does not
/// accept items.
///
/// A control whose count is genuinely zero after a successful add is not
/// distinguishable from a rejection by the return value alone; use
/// [`rw_widget_list_count`] to confirm when that matters.
///
pub extern "C" fn rw_widget_list_add(widget_id: u64, text: *const c_char) -> c_uint {
    c_try!({
        let item = c_str_or_default(text);
        // One registry round trip, not two: the count is read from the same exclusive
        // borrow that appended, instead of re-entering the registry through
        // `rw_widget_list_count` afterwards (a second detach/reinsert cycle).
        let outcome = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            // Moved, not cloned: `with_widget_mut` takes an `FnOnce`, so the old `item.clone()`
            // allocated a second string that was immediately dropped.
            let added = crate::widget::capability::append_widget_list_item(widget, item);
            let count =
                if added { crate::widget::capability::widget_list_item_count(widget) } else { 0 };
            (added, count)
        })
        .unwrap_or((false, 0));
        let (added, count) = outcome;
        if !added {
            return 0;
        }
        crate::widget::runtime::request_repaint(widget_id);
        count as c_uint
    })
}

#[no_mangle]
/// Removes every item from a list-like control.
///
/// Returns `false` when the control does not hold items, so a caller can tell
/// "cleared" from "this control has no collection to clear".
///
pub extern "C" fn rw_widget_list_clear(widget_id: u64) -> CBool {
    c_try!({
        let cleared = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            crate::widget::capability::clear_widget_list_items(widget)
        })
        .unwrap_or(false);
        if cleared {
            crate::widget::runtime::request_repaint(widget_id);
        }
        cleared
    })
}

#[no_mangle]
/// Returns how many items a list-like control holds, or `0` when it holds none or
/// does not hold items.
///
pub extern "C" fn rw_widget_list_count(widget_id: u64) -> c_uint {
    c_try!({
        crate::widget::runtime::with_widget(widget_id, |widget| {
            crate::widget::capability::widget_list_item_count(widget)
        })
        .unwrap_or(0) as c_uint
    })
}

#[no_mangle]
/// Returns the number of bytes item `index` needs, writing its text into `out`.
///
/// # Why this exists
///
/// `item_count` was the only collection fact a caller could read across the C ABI. A
/// host could add items, count them and clear them but never read back what it had
/// added, so a list's *contents* were write-only. This is the read side.
///
/// # Reading the result
///
/// Follows the same two-call convention as the other enumeration entry points: call
/// with `out = NULL` or `cap = 0` to learn the required length, then again with a
/// buffer. The return is always the full byte length.
///
/// A **zero** return means "no item at this index", which covers an out-of-range
/// index and a control that holds no items. It is not distinguishable from an empty
/// string here; a caller that needs to tell them apart compares against
/// [`rw_widget_list_count`].
///
pub extern "C" fn rw_widget_list_item(
    widget_id: u64,
    index: c_uint,
    out: *mut c_char,
    cap: c_uint,
) -> c_uint {
    c_try!({
        let text = crate::widget::runtime::with_widget(widget_id, |widget| {
            crate::widget::capability::widget_list_item(widget, index as usize)
        })
        .flatten();
        match text {
            Some(text) => write_c_string(&text, out, cap),
            None => 0,
        }
    })
}

#[no_mangle]
/// Creates a layout for `parent` and stores it, replacing any previous layout.
///
/// `kind_name` is one of `"hbox"` / `"vbox"` / `"grid"` / `"uniform_grid"` /
/// `"stack"` / `"splitter"` / `"form"` / `"flow"` / `"wrap"` / `"flex"` /
/// `"border"` / `"center"` / `"aspect_ratio"` / `"keyboard_aware"`; it matches
/// the spelling a declarative document uses, so the same name works in JSON and
/// here. `spacing` and `margin` are applied where the kind uses them, and are
/// ignored by the kinds that do not.
///
/// Returns `false` for an unknown kind or an unknown widget. An unknown kind is
/// refused rather than defaulting: a caller that misspells `"hbox"` would otherwise
/// get a vertical box and a layout that looks wrong for reasons it cannot see.
///
/// # Why a layout is stored rather than returned as a handle
///
/// The layout lives per-parent in the runtime registry, which is also where
/// [`rw_widget_layout_add`] and [`rw_widget_layout_apply`] look. Handing a pointer
/// across the ABI would make the caller responsible for a `Box<dyn Layout>` whose
/// lifetime is tied to the parent — a contract no C API can express without a
/// destructor and a nullability story. The parent id is the handle.
///
/// # Safety
///
/// `kind_name` must be null or point to a NUL-terminated string.
///
pub unsafe extern "C" fn rw_widget_set_layout(
    parent: u64,
    kind_name: *const c_char,
    spacing: c_int,
    margin: c_int,
) -> CBool {
    c_try!({
        let name = c_str_or_default(kind_name);
        if !crate::widget::runtime::is_mounted(parent) {
            crate::error::ffi::record_capability_error(
                crate::widget::capability::CapabilityAccessError::UnknownWidget,
            );
            return false;
        }
        // `crate::json` is gated on the JSON engine (a device profile building with
        // the JSON surface). A build with this ABI but without it has no layout-kind
        // parser, so the honest answer is a recorded refusal rather than a link
        // error; the symbol stays available either way (principle #41).
        //
        // The spec is built **inside** the branch that reads it. It used to be built
        // unconditionally above the `#[cfg]`, which is a build error rather than a dead value
        // whenever `serde_json` is not a feature of this build: `serde_json::json!` names the
        // crate in the macro expansion, and the crate is not linked. That combination
        // (this ABI, no `serde_json`) was never built by a gate — `check_profiles.sh` passes
        // `serde_json` alongside `mobile-api` — so the unconditional build looked fine while a
        // profile list without it could not compile at all. Constructing it here makes the two
        // branches independent, which is what the `#[cfg]` was meant to express.
        #[cfg(all(device_profile, feature = "serde_json"))]
        {
            let spec = serde_json::json!({
                "type": name,
                "spacing": spacing,
                "margin": margin,
            });
            match crate::json::parse_layout_kind(&spec)
                .map(|kind| crate::json::create_layout_from_kind(&kind))
            {
                Ok(layout) => {
                    crate::layout::declarative::store_layout(parent, layout);
                    true
                }
                Err(message) => {
                    crate::error::ffi::record_message_error(&message);
                    false
                }
            }
        }
        #[cfg(not(all(device_profile, feature = "serde_json")))]
        {
            // The three parameters are still read so their presence in the signature is not a
            // silent lie about being used; the refusal is what the caller observes.
            let _ = (name, spacing, margin);
            crate::error::ffi::record_message_error(
                "this build has no JSON layout engine, so a declarative layout kind cannot be \
                 created here",
            );
            false
        }
    })
}

#[no_mangle]
/// Adds `child` to the layout stored for `parent`, with `stretch`.
///
/// `stretch` is clamped to a minimum of `1`, because `0` is not a valid stretch in
/// the layout implementations and would make the child invisible — a silent
/// disappearance is worse than a substituted default.
///
/// Returns `false` when `parent` has no layout or either id is unknown, so a caller
/// cannot believe a child was registered when it was not.
///
pub extern "C" fn rw_widget_layout_add(parent: u64, child: u64, stretch: c_uint) -> CBool {
    c_try!({
        let added = crate::layout::declarative::add_widget_to_layout(child, stretch.max(1), parent);
        if !added {
            crate::error::ffi::record_message_error(&format!(
                "no layout is stored for widget {parent}; call rw_widget_set_layout first"
            ));
        }
        added
    })
}

#[no_mangle]
/// Registers a stretchable spacer with the layout stored for `parent`.
///
/// See `crate::layout::declarative::SPACER_ID` for how a spacer is represented. A
/// spacer is what makes "push these two buttons apart" expressible without an empty
/// widget, so the ABI exposes it rather than requiring a stretch on a real control.
///
/// Returns `false` when `parent` has no layout.
///
pub extern "C" fn rw_widget_layout_add_spacer(parent: u64, stretch: c_uint) -> CBool {
    c_try!(crate::layout::declarative::add_spacer_to_layout(stretch.max(1), parent))
}

#[no_mangle]
/// Removes `child` from the layout stored for `parent`.
///
/// Returns `false` when `parent` has no layout. Removing a child that is not in the
/// layout still reports `true`: the layout was reached and told, and the caller's
/// intent — "this child must not be laid out" — holds either way.
///
pub extern "C" fn rw_widget_layout_remove(parent: u64, child: u64) -> CBool {
    c_try!(crate::layout::declarative::remove_widget_from_layout(child, parent))
}

#[no_mangle]
/// Discards the layout stored for `parent`.
///
/// Returns `false` when there was none. A host unmounting a container should call
/// this, or the registry keeps a layout whose children no longer exist.
///
pub extern "C" fn rw_widget_layout_clear(parent: u64) -> CBool {
    c_try!(crate::layout::declarative::forget_layout(parent))
}

#[no_mangle]
/// Recomputes the layout for `parent` inside `x`, `y`, `width`, `height`, and moves
/// its children accordingly.
///
/// Returns how many children were positioned. `0` when no layout is stored, or when
/// the layout has no children — up to the caller which of those it was.
///
/// # Why the caller supplies the rectangle
///
/// A layout positions children *within* a rectangle. Taking it from the parent's own
/// geometry would be wrong for a container that is being laid out by an enclosing
/// layout, and unknowable for one that is not mounted yet.
///
pub extern "C" fn rw_widget_layout_apply(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> c_uint {
    c_try!({
        let rect = crate::core::Rect::new(x, y, width, height);
        let applied = crate::layout::declarative::apply_layout(parent, rect);
        if !applied.is_empty() {
            crate::widget::runtime::request_repaint(parent);
        }
        applied.len() as c_uint
    })
}

#[no_mangle]
/// Counts the children in the layout stored for `parent`, or `0` when there is none.
///
/// Lets a caller confirm registration without applying it, which is what a host needs
/// while it is still building the tree.
///
pub extern "C" fn rw_widget_layout_child_count(parent: u64) -> c_uint {
    c_try!({ crate::layout::declarative::layout_child_count(parent) as c_uint })
}

#[no_mangle]
/// Applies one style declaration to a widget, written as `"property: value"`.
///
/// Uses the same property names and value syntax as a stylesheet —
/// `"background-color: #FF0000"`, `"border-radius: 4"`, `"font-size: 14"` — and
/// routes through the same parser, so a value accepted here is accepted in CSS and
/// vice versa. That is the whole point: a second parser would eventually disagree
/// with the first.
///
/// Returns `false` when the widget is unknown, the declaration is malformed, or the
/// value cannot be parsed for that property. `rw_error_message` carries the parser's
/// own text, which names the offending property or value.
///
/// # Why this is not `rw_set_widget_property`
///
/// They deliberately differ. The property layer writes through each control's
/// `WidgetProperties` contract, which describes what the control *is* — `value`,
/// `items`, `checked`. This writes through the style pipeline, which describes how
/// the control *looks*. A caller setting `color` wants the latter even when a control
/// happens to expose a same-named property, so the two entry points stay separate
/// rather than one guessing the other's intent.
///
/// # Safety
///
/// `declaration` must be null or point to a NUL-terminated string.
///
pub unsafe extern "C" fn rw_widget_set_style(widget_id: u64, declaration: *const c_char) -> CBool {
    c_try!({
        let text = c_str_or_default(declaration);
        let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            // Reject an unknown property before applying, because `apply_one` ignores
            // one by design (the CSS spec requires a stylesheet to tolerate properties
            // it does not know). A single programmatic write has no such excuse: the
            // caller typed one property, and a misspelling must not report success.
            let property = text.split(':').next().unwrap_or("").trim();
            if !property.is_empty() && !crate::style::CssParser::is_known_property(property) {
                return Err(format!(
                    "unknown style property {property:?}; see the styling chapter of the cookbook \
                     for the accepted names"
                ));
            }
            let mut style = widget.style().clone();
            match crate::style::CssParser::apply_declaration_text(&text, &mut style) {
                Ok(()) => {
                    widget.set_style(style);
                    Ok(())
                }
                Err(message) => Err(message),
            }
        });
        match applied {
            Some(Ok(())) => {
                crate::widget::runtime::request_repaint(widget_id);
                true
            }
            Some(Err(message)) => {
                crate::error::ffi::record_message_error(&message);
                false
            }
            None => {
                crate::error::ffi::record_capability_error(
                    crate::widget::capability::CapabilityAccessError::UnknownWidget,
                );
                false
            }
        }
    })
}
