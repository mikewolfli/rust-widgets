// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared dialog chrome: label resolution, the modal scrim, and the reveal geometry.
//!
//! Kept out of `mod.rs` (principle #8). Six dialogs build an action row from `tr!` sites and
//! every modal dims the page behind it, so these bodies live in one file rather than being
//! repeated — which is how six of them came to differ in the first place.

use crate::compat::{RefCell, Vec};
use crate::core::{Color, ObjectId, Rect};
use crate::render::RenderContext;

/// The built-in English labels for the dialog catalogue keys.
///
/// # Why a table in addition to the catalogue
///
/// The catalogue is the translation resource table: when it is initialised it wins, and the
/// embedded English file is loaded on first use. A build without the `i18n` capability has no
/// catalogue at all, so this table is the resource table for that build — the labels live in
/// one place instead of being restated at each `localized_label` call site, which is how two
/// dialogs came to spell "OK" differently.
const DIALOG_RESOURCE_FALLBACKS: &[(&str, &str)] = &[
    ("common.button.ok", "OK"),
    ("common.button.cancel", "Cancel"),
    ("common.button.open", "Open"),
    ("common.button.save", "Save"),
    ("common.button.close", "Close"),
    ("dialog.ok", "OK"),
    ("dialog.cancel", "Cancel"),
    ("dialog.message_box.show_details", "Show Details..."),
    ("dialog.message_box.hide_details", "Hide Details..."),
];

/// The English label this module ships for `key`, if it has one.
fn resource_fallback(key: &str) -> Option<&'static str> {
    DIALOG_RESOURCE_FALLBACKS
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, label)| *label)
}

/// Resolves a dialog label for **display**: the catalogue's translation when it has one, the
/// built-in resource label for the key, and the English label otherwise.
///
/// # Why the key is not an acceptable fallback
///
/// `translate_key` returns the key itself when no catalogue is initialised (or the key is
/// absent). Drawing that value is how a message box's OK button came to read `common....`: the
/// row reserved 80px for "OK", and the raw key `common.button.ok` was elided to fit. A dialog's
/// buttons are user-facing copy, so the fallback is the English label the enum already carries —
/// never the catalogue key.
///
/// # Why this is one function rather than each dialog's own check
///
/// Six dialogs build their action row from `tr!` sites, and a per-site `== key` comparison is a
/// rule that has to be repeated exactly. One helper means a new dialog inherits the fallback, and
/// a catalogue that is initialised later still resolves correctly because the lookup happens when
/// the label is drawn.
pub(crate) fn localized_label(key: &str, english: &str) -> crate::compat::String {
    #[cfg(feature = "i18n")]
    {
        let translated = crate::i18n::translate(key);
        if translated.is_empty() || translated == key {
            crate::compat::String::from(resource_fallback(key).unwrap_or(english))
        } else {
            translated
        }
    }
    #[cfg(not(feature = "i18n"))]
    {
        // No catalogue exists in this build, so the built-in resource table and then the
        // caller's English label are the whole answer. Going through `translate` here would
        // log a warning on every frame for a case that is not a failure.
        let _ = key;
        crate::compat::String::from(resource_fallback(key).unwrap_or(english))
    }
}

/// The focus targets of a dialog, in tab order.
///
/// # Why a shared iterator and not a per-dialog loop
///
/// Six self-drawn dialogs move a focus ring across their own controls with the same rule:
/// Tab advances, Shift+Tab retreats, and the first/last target wraps. Spelling that rule in
/// each `handle_event` is how the dialogs came to disagree about whether Tab moves forward or
/// whether it wraps at all (issue DLG-10), so the chain and the step live here once.
pub(crate) fn focus_chain(count: usize) -> core::ops::Range<usize> {
    0..count
}

/// The focus index `forward` steps from `current` within a chain of `count` targets.
///
/// Returns `None` when there is nothing to focus. A `current` that is out of range is treated
/// as "before the first" when moving forward and "after the last" when moving back, so a chain
/// that shrank (a hidden button) cannot leave the ring on an index that no longer exists.
pub(crate) fn move_focus(current: Option<usize>, count: usize, forward: bool) -> Option<usize> {
    // `focus_chain` is the one declaration of "which indices are focusable", so a caller that
    // needs the list and this helper cannot disagree about its length.
    let count = focus_chain(count).count();
    if count == 0 {
        return None;
    }
    let next = match current {
        Some(index) if index < count => {
            if forward {
                (index + 1) % count
            } else {
                (index + count - 1) % count
            }
        }
        Some(_) if forward => 0,
        Some(_) => count - 1,
        None if forward => 0,
        None => count - 1,
    };
    Some(next)
}

// The scrim stack: which modal currently owns the one page-dimming plane.
//
// # Why the stack lives here and not on the control
//
// DLG-001 recorded two halves of the same defect. The plane was painted over each dialog's
// **own** rectangle, so it dimmed the area behind that dialog rather than the page; and the
// eight modal dialogs plus `ModalHost` each decided independently whether to paint it, so two
// dialogs opened together stacked two planes that knew nothing about each other.
//
// The window layer is the right owner for both (that is what `ModalHost` already is), but
// moving the eight controls onto a host they are not mounted under would change their public
// mounting contract. Until the window hook exists, this registry is the single entry every
// modal in the family calls: the **page** is resolved from the root the control hangs under
// (see [`modal_page_rect`]) and the **owner** is the last modal to register. The owning modal
// paints one full-page plane; the ones below it paint none.
//
// The remaining window-layer hook is outside `src/widget/dialog/` and is recorded precisely
// rather than silently half-implemented:
//
// * `src/widget/runtime.rs:6485` (`render_frame_tree`) paints the root and each child, and
//   `paint_widget_direct` at `6821` has no scrim pass — a true window layer would paint the
//   plane once per window instead of once per owning control.
// * `src/lib.rs:2328` (`set_widget_visible`) is the show path six of the eight dialogs use;
//   a push hook there would make their open order explicit instead of first-paint order.
thread_local! {
    static SCRIM_STACK: RefCell<Vec<ObjectId>> = const { RefCell::new(Vec::new()) };
    /// The modal whose scrim plane the current frame's root pass already painted (DLG-001).
    ///
    /// A modal painted as one of the root's children is rasterised into its own layer, and its
    /// scrim is clipped to that layer — so the plane has to be painted by the root pass, before
    /// the modal, or the page under it keeps its full brightness. Painting it in both places dims
    /// the page twice; this flag is how the modal's own `draw` knows the plane is already down.
    static PLANE_PAINTED: core::cell::Cell<Option<ObjectId>> = const { core::cell::Cell::new(None) };
}

/// Clears the "plane already painted" record for a new frame.
pub(crate) fn reset_plane_painted() {
    PLANE_PAINTED.with(|slot| slot.set(None));
}

/// Records that this frame's root pass painted the plane for `owner`.
pub(crate) fn note_plane_painted(owner: ObjectId) {
    PLANE_PAINTED.with(|slot| slot.set(Some(registry_id(owner))));
}

/// Whether this frame's root pass already painted the plane for `modal`.
fn plane_already_painted(modal: ObjectId) -> bool {
    PLANE_PAINTED.with(|slot| slot.get() == Some(registry_id(modal)))
}

/// How far [`modal_page_rect`] walks before it stops trusting the parent chain.
///
/// A widget tree is shallow (window → containers → controls); the bound exists for the same
/// reason `runtime::TOP_WINDOW_MAX_DEPTH` does — a corrupted chain is answered with the
/// fallback rather than a hang.
const MODAL_ROOT_WALK_LIMIT: usize = 64;

/// The page a modal dims: the client box of the root the control hangs under.
///
/// # Why the control's own rectangle is the fallback
///
/// An unmounted control (a test-held dialog, a control created before it is placed) has no
/// root to ask, and its own rectangle is the only area it can be drawn into. That fallback is
/// also the historical behaviour, so a caller that never mounts its dialogs sees no change.
///
/// # Why `parent` is passed rather than read from the registry
///
/// The call comes from the control's own `draw`, where the runtime has lent the widget out
/// exclusively: `read_widget(id)` answers `None` for it by design. The parent is already known
/// to the control (`BaseWidget::parent`), and every step above it is an ordinary read.
pub(crate) fn modal_page_rect(control: Rect, parent: Option<ObjectId>) -> Rect {
    let Some(mut current) = parent else {
        return control;
    };
    for _ in 0..MODAL_ROOT_WALK_LIMIT {
        let Some(next) = crate::widget::runtime::read_widget(current, |widget| widget.parent())
        else {
            return control;
        };
        match next {
            Some(parent) => current = parent,
            None => {
                // The root: a window's geometry is in operating-system space while its children
                // are in client coordinates, so the client page is the root's **size** at the
                // origin. A surface is drawn the same way by its own render pass.
                return crate::widget::runtime::geometry_of(current)
                    .map(|root| Rect::new(0, 0, root.width, root.height))
                    .unwrap_or(control);
            }
        }
    }
    control
}

/// Whether `modal` still belongs on the scrim stack.
///
/// A mounted control answers through its own visibility. An id the registry cannot read is
/// either unmounted (a test-held control, a control that has not been placed yet) or lent out
/// because it is being painted right now — and there is no answer that distinguishes the two,
/// so the entry is kept: dropping a live modal would let an older one paint over it.
fn scrim_is_live(modal: ObjectId) -> bool {
    crate::widget::runtime::read_widget(modal, |widget| widget.is_visible()).unwrap_or(true)
}

/// Registers `modal` and answers whether it owns the plane.
///
/// A repeated draw does **not** re-raise: the order is the open order (or, for a dialog with no
/// open hook, the order it first painted), not the order the frame happens to visit it. Stale
/// entries are pruned in the same pass, so a closed modal cannot keep a live one from owning the
/// plane.
fn claim_scrim(modal: ObjectId) -> bool {
    let modal = registry_id(modal);
    SCRIM_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        stack.retain(|id| *id == modal || scrim_is_live(*id));
        if !stack.contains(&modal) {
            stack.push(modal);
        }
        stack.last().copied() == Some(modal)
    })
}

/// Pushes `modal` as the scrim owner, whether or not it has been painted yet.
///
/// [`Dialog::open`](super::dialog_widget::Dialog::open) calls this so a dialog that is opened
/// second owns the plane from the instant it opens, rather than from the first frame it paints.
pub(crate) fn push_modal_layer(modal: ObjectId) {
    let modal = registry_id(modal);
    SCRIM_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        stack.retain(|id| *id != modal);
        stack.push(modal);
    });
}

/// Removes `modal` from the scrim stack, returning ownership to whatever is below it.
pub(crate) fn pop_modal_layer(modal: ObjectId) {
    let modal = registry_id(modal);
    SCRIM_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        if let Some(position) = stack.iter().rposition(|id| *id == modal) {
            stack.remove(position);
        }
    });
}

/// Paints the modal scrim over the page `control` sits on.
///
/// `control` is the dialog's own rectangle and `parent` its direct parent; together they resolve
/// the root client box through [`modal_page_rect`]. Only the top modal in the shared stack
/// paints, so two dialogs opened together do not stack two dimming planes: the modal that
/// opened last owns the page.
///
/// # Why this is one function and not eight copies
///
/// Every modal dialog in this module dims the page it sits on, and six of them were silently
/// missing it: `Dialog` and `MessageBox` were fixed first, then the scan showed
/// `color_dialog` / `file_dialog` / `font_dialog` / `input_dialog` / `progress_dialog` carried a
/// `modal` flag with nothing painted for it. Six copies of the same six lines is how six of them
/// came to diverge in the first place, so the body lives here once.
///
/// # Why the `Scrim` role rather than a blend
///
/// BLUE21 B23 recorded that blending toward the ink **lightens** a dark page — a scrim that
/// brightens what it covers is the defect stated in `Colors::scrim`'s own documentation. The
/// role lets a dark theme dim deliberately while a light theme darkens, and the literal survives
/// only as the fallback for a theme that predates the role.
pub(crate) fn draw_modal_scrim(
    context: &mut RenderContext,
    modal: ObjectId,
    control: Rect,
    parent: Option<ObjectId>,
) {
    draw_modal_scrim_scaled(context, modal, control, parent, 1.0);
}

/// Paints the modal scrim at `strength` of its resolved opacity, for a backdrop that fades in.
///
/// Scaling the **alpha** rather than blending toward the page is the one form that is right for
/// both a dark scrim (which must get darker as it arrives) and a light one (which must get lighter):
/// a blend toward the window fill would move a scrim in the wrong direction in one of the two
/// appearances. `strength` is clamped, and the whole plane is skipped at zero so a hidden dialog
/// leaves the page untouched (and leaves the stack, so the modal below it owns the plane again).
pub(crate) fn draw_modal_scrim_scaled(
    context: &mut RenderContext,
    modal: ObjectId,
    control: Rect,
    parent: Option<ObjectId>,
    strength: f32,
) {
    draw_modal_scrim_tinted(context, modal, control, parent, strength, None);
}

/// Paints the modal scrim in `tint`, or in the theme's `Scrim` role when `tint` is `None`.
///
/// `ModalHost` keeps its `scrim_color` property, so it paints through this entry rather than the
/// theme-only one. Both entries share the same stack, which is what stops a host and a dialog
/// from each dimming the page once (DLG-001).
pub(crate) fn draw_modal_scrim_tinted(
    context: &mut RenderContext,
    modal: ObjectId,
    control: Rect,
    parent: Option<ObjectId>,
    strength: f32,
    tint: Option<Color>,
) {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        pop_modal_layer(modal);
        return;
    }
    let page = modal_page_rect(control, parent);
    if page.width == 0 || page.height == 0 {
        return;
    }
    if !claim_scrim(modal) {
        return;
    }
    // The root pass paints the plane for a modal that is one of its painted children, because a
    // child's layer clips its own scrim to its rectangle. Claiming above still ordered the stack.
    if plane_already_painted(modal) {
        return;
    }
    let scrim = tint.unwrap_or_else(|| {
        crate::style::layer_color(crate::style::LayerColor::Scrim)
            .unwrap_or_else(|| Color::BLACK.with_alpha(82))
    });
    let scrim = scrim.with_alpha((scrim.a as f32 * strength) as u8);
    context.fill_rect(page, scrim);
}

/// The scrim every modal family paints when it does not name its own tint.
///
/// Shared with the window layer so the plane a host paints for a modal that is not one of its
/// painted children is the same colour as the per-control plane (DLG-001).
pub(crate) fn scrim_plane_color() -> Color {
    crate::style::layer_color(crate::style::LayerColor::Scrim)
        .unwrap_or_else(|| Color::BLACK.with_alpha(82))
}

/// The id the widget runtime knows `id` by.
///
/// A control's `BaseWidget` id is its own construction-time id; the registry mounts it under a
/// separate `ObjectId` and keeps the mapping in `runtime::registry_id_of`. The scrim stack has to
/// speak the registry's language: `scrim_owner_for_root` is asked about a registry id (the root
/// being painted), and `modal_root` walks parent links that are already registry ids. Storing the
/// own id made those two answers incomparable, so a modal mounted as its own surface never dimmed
/// its window at all.
fn registry_id(id: ObjectId) -> ObjectId {
    crate::widget::runtime::registry_id_of(id).unwrap_or(id)
}

/// The modal that owns the scrim plane for `root`, if any (DLG-001).
///
/// A modal is associated with a window through its parent chain, which is the same relation
/// [`modal_page_rect`] walks to find the page it dims. The window layer uses this to answer
/// "does this frame owe a scrim?" for a modal it does not paint as one of its children (a
/// dialog mounted as its own surface).
pub(crate) fn scrim_owner_for_root(root: ObjectId) -> Option<ObjectId> {
    let root = registry_id(root);
    SCRIM_STACK.with(|stack| {
        stack
            .borrow()
            .iter()
            .rev()
            .copied()
            .find(|modal| scrim_is_live(*modal) && modal_root(*modal) == Some(root))
    })
}

/// The root of `id`'s parent chain, or `None` when it has no parent.
fn modal_root(id: ObjectId) -> Option<ObjectId> {
    let mut current =
        crate::widget::runtime::read_widget(id, |widget| widget.parent()).flatten()?;
    for _ in 0..MODAL_ROOT_WALK_LIMIT {
        match crate::widget::runtime::read_widget(current, |widget| widget.parent()).flatten() {
            Some(parent) => current = parent,
            // An unreadable node is the root being painted (it is lent out during its own
            // frame), which is exactly the answer wanted here.
            None => return Some(current),
        }
    }
    None
}

/// The scrim owners, bottom first. Test-only: production code only needs "am I the owner".
#[cfg(test)]
pub(crate) fn scrim_stack_snapshot() -> Vec<ObjectId> {
    SCRIM_STACK.with(|stack| stack.borrow().clone())
}

/// How small a revealing modal is at the instant it appears: 0.92 of its settled size.
///
/// A dialog that grows from nothing reads as a pop rather than an arrival, and one that grows from
/// far away reads as a zoom unrelated to a dialog's purpose. A few percent is the "settle" every
/// platform uses; the value is deliberately *not* zero, so the first visible frame already shows the
/// dialog's shape. Shared by every modal family so a dialog and a message box arrive alike — two
/// nearby literals is exactly how the eight dialogs acquired four title-bar heights (rule #101).
pub(crate) const REVEAL_MIN_SCALE: f32 = 0.92;

/// Scales `rect` about its own centre, rounding outward so a shrinking frame never loses a pixel
/// column to truncation while it is still meant to be visible.
///
/// The frame is grown from its centre rather than from a corner because a dialog that grows from a
/// corner reads as a panel sliding in, which is a different affordance (that is what `bottom_sheet`
/// and `navigation_drawer` do). Kept as a free function so a draw path has one expression for the
/// geometry and a test can compose the same rect without duplicating the arithmetic.
pub(crate) fn scale_about_centre(rect: Rect, scale: f32) -> Rect {
    let scale = scale.clamp(0.0, 1.0);
    let width = ((rect.width as f32 * scale) + 0.5) as u32;
    let height = ((rect.height as f32 * scale) + 0.5) as u32;
    let dx = (rect.width.saturating_sub(width) / 2) as i32;
    let dy = (rect.height.saturating_sub(height) / 2) as i32;
    Rect::new(rect.x + dx, rect.y + dy, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_key_uses_the_shipped_resource_label() {
        // The embedded catalogue is loaded on first use, so this is the resource-table path:
        // an explicitly different English argument must not leak past a known key.
        assert_eq!(localized_label("common.button.ok", "Wrong"), "OK");
    }

    #[test]
    fn an_unknown_key_falls_back_to_the_callers_english() {
        assert_eq!(localized_label("no.such.dialog.key", "Custom"), "Custom");
    }

    #[test]
    fn the_resource_table_covers_every_shipped_key_with_a_label() {
        for (key, label) in DIALOG_RESOURCE_FALLBACKS {
            assert!(!key.is_empty(), "a resource key cannot be empty");
            assert!(!label.is_empty(), "{key} must have a label");
            assert_eq!(resource_fallback(key), Some(*label));
        }
    }

    /// A later modal owns the single scrim, and closing it hands ownership back down.
    ///
    /// # The defect this pins
    ///
    /// The eight dialogs painted one scrim each from their own rectangles, so two dialogs open
    /// together produced two overlapping planes and neither stack existed to say which was on
    /// top. The registry is the one statement of that order, so it is asserted directly rather
    /// than inferred from pixels.
    #[test]
    fn a_later_modal_owns_the_only_scrim() {
        const FIRST: ObjectId = 0xA1;
        const SECOND: ObjectId = 0xA2;

        push_modal_layer(FIRST);
        assert!(claim_scrim(FIRST), "the first modal owns the plane");

        push_modal_layer(SECOND);
        assert!(!claim_scrim(FIRST), "a modal below the top must not paint a second plane");
        assert!(claim_scrim(SECOND), "the later modal owns the plane");

        pop_modal_layer(SECOND);
        assert!(claim_scrim(FIRST), "closing the top hands ownership back down");
        pop_modal_layer(FIRST);
    }

    /// Re-painting a modal must not raise it above one opened later.
    #[test]
    fn a_repeated_paint_does_not_reorder_the_scrim_stack() {
        const BOTTOM: ObjectId = 0xB1;
        const TOP: ObjectId = 0xB2;

        push_modal_layer(BOTTOM);
        push_modal_layer(TOP);
        // A bottom-up paint order visits BOTTOM first every frame; the stack keeps the open order.
        assert!(!claim_scrim(BOTTOM));
        assert_eq!(SCRIM_STACK.with(|stack| stack.borrow().clone()), vec![BOTTOM, TOP]);
        assert!(claim_scrim(TOP));

        pop_modal_layer(TOP);
        pop_modal_layer(BOTTOM);
        assert!(SCRIM_STACK.with(|stack| stack.borrow().is_empty()));
    }

    /// A window host and the dialog it hosts must not each paint the page plane.
    ///
    /// The host registers its own id when it paints; a dialog registered underneath it must stop
    /// painting, and must take the plane back when the host stops (DLG-001).
    #[test]
    fn a_host_and_its_dialog_share_one_owner() {
        const HOST: ObjectId = 0xC1;
        const DIALOG: ObjectId = 0xC2;

        push_modal_layer(DIALOG);
        push_modal_layer(HOST);
        assert!(!claim_scrim(DIALOG), "the dialog must not paint under the host's plane");
        assert!(claim_scrim(HOST), "the host owns the one plane");

        pop_modal_layer(HOST);
        assert!(claim_scrim(DIALOG), "the dialog owns the plane once the host stops painting");
        pop_modal_layer(DIALOG);
    }

    /// An unmounted control has no root, so its own rectangle is the page it dims.
    #[test]
    fn an_unmounted_modal_dims_its_own_rectangle() {
        let control = Rect::new(10, 20, 200, 120);
        assert_eq!(modal_page_rect(control, None), control);
        // An id with no registry entry (a test-held control) answers the same way through the
        // parent walk: there is no root to resolve.
        assert_eq!(modal_page_rect(control, Some(0xDEAD_BEEF)), control);
    }

    /// A mounted modal dims the client box of the root it hangs under, not its own rectangle.
    #[test]
    fn a_mounted_modal_dims_the_root_client_box() {
        use crate::widget::{runtime, Dialog, Window};

        let window = runtime::register(Box::new(Window::new(
            "scrim-page".to_string(),
            Rect::new(140, 140, 640, 480),
        )))
        .expect("mount window");
        let dialog = runtime::register(Box::new(Dialog::new(Rect::new(20, 30, 240, 160))))
            .expect("mount dialog");
        runtime::with_widget_mut(window, |widget| widget.base_mut().add_child_linked(dialog))
            .expect("window is mounted");

        let own = runtime::geometry_of(dialog).expect("dialog geometry");
        let page = modal_page_rect(own, Some(window));
        assert_eq!(
            page,
            Rect::new(0, 0, 640, 480),
            "the page is the window's client box, not the dialog's own rect: {page:?}"
        );

        runtime::unregister(dialog);
        runtime::unregister(window);
    }

    /// DLG-001: a modal this frame paints is dimmed exactly once.
    ///
    /// The dialog paints its own scrim over the page; the window pass used to paint the same
    /// plane again, which showed as a visibly darker page. The window pass is only owed when the
    /// owner is not among this frame's painted subtrees. Two identical windows are rendered — one
    /// bare, one with a settled modal linked as its child — so the page pixel can be compared
    /// against a single blend of the scrim over the measured base colour, with the double blend
    /// asserted different so the comparison cannot pass vacuously.
    #[test]
    fn a_modal_painted_by_this_frame_is_dimmed_once() {
        use crate::core::{Color, Size};
        use crate::widget::capability::coercion::widget_as_mut;
        use crate::widget::{runtime, Dialog, Window};

        const PAGE: (u32, u32) = (320, 240);
        let size = Size::new(PAGE.0, PAGE.1);
        // A page pixel far from where the card will sit, so the scrim is the only thing on it.
        let (x, y) = (300u32, 220u32);
        let index = (((y * PAGE.0) + x) * 4) as usize;
        let pixel = |frame: &[u8]| Color::rgb(frame[index], frame[index + 1], frame[index + 2]);

        let bare = runtime::register(Box::new(Window::new(
            "scrim-bare".to_string(),
            Rect::new(0, 0, PAGE.0, PAGE.1),
        )))
        .expect("mount bare window");
        let base = pixel(&runtime::render_frame_tree(bare, size, Color::WHITE).expect("bare frame"));

        let window = runtime::register(Box::new(Window::new(
            "scrim-modal".to_string(),
            Rect::new(0, 0, PAGE.0, PAGE.1),
        )))
        .expect("mount modal window");
        let dialog = runtime::register(Box::new(Dialog::new(Rect::new(20, 20, 120, 80))))
            .expect("mount dialog");
        runtime::with_widget_mut(dialog, |widget| {
            let dialog = widget_as_mut::<Dialog>(widget).expect("the registered type is a Dialog");
            dialog.open();
            // Settle the reveal unconditionally: the transition only reports movement once a
            // tick has advanced it, so a `while is_animating()` loop can exit before the first
            // frame and leave the scrim at zero strength.
            for _ in 0..40 {
                dialog.tick(16);
            }
            assert_eq!(dialog.reveal_progress(), 1.0, "the modal is fully revealed");
        })
        .expect("dialog is mounted");
        runtime::with_widget_mut(window, |widget| widget.base_mut().add_child_linked(dialog))
            .expect("window is mounted");

        let frame = runtime::render_frame_tree(window, size, Color::WHITE).expect("modal frame");
        let page = pixel(&frame);
        let scrim = scrim_plane_color();
        let weight = f32::from(scrim.a) / 255.0;
        let once = base.blend(&scrim, weight);
        let twice = once.blend(&scrim, weight);
        let close = |left: Color, right: Color| {
            (i32::from(left.r) - i32::from(right.r)).abs() <= 2
                && (i32::from(left.g) - i32::from(right.g)).abs() <= 2
                && (i32::from(left.b) - i32::from(right.b)).abs() <= 2
        };
        assert!(close(page, once), "the page is dimmed by one scrim: {page:?} vs {once:?}");
        assert!(!close(page, twice), "and not by two: {page:?} vs {twice:?}");

        runtime::unregister(dialog);
        runtime::unregister(window);
        runtime::unregister(bare);
    }
}
