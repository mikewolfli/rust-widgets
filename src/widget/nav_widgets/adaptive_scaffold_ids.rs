// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfi/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Object-id content slots for [`AdaptiveScaffold`] (issue-20261104 DM-06/DM-08).
//!
//! Split out of `adaptive_scaffold.rs` to keep that file inside the source-structure limit
//! (principle #9). A declared child is mounted by the runtime, which owns it, so a JSON
//! document can only hand over its id. These methods record those ids, and
//! [`AdaptiveScaffold::sync_id_slots`] places them in the same bands the `Box` slots use.

use super::*;

impl AdaptiveScaffold {
    // ── Object-id content slots (issue-20261104 DM-06/DM-08) ────────────────
    //
    // The `set_*` family above takes ownership of a `Box<dyn Widget>`, which a declaration
    // cannot do: the loader mounts a node through the runtime, and the runtime keeps the
    // widget. These setters record the id instead. The slot's band and visibility are
    // applied by [`Self::sync_id_slots`] at paint time — the same moment the `Box` slots get
    // theirs — so a declared page moves and hides exactly like a hand-built one.

    /// Sets the body page by id.
    ///
    /// An id slot wins over a `Box` slot for the same position: the id is the one a
    /// document declared, and keeping both would paint two bodies into one band.
    pub fn set_body_id(&mut self, widget: Option<ObjectId>) {
        self.body_id = self.adopt_id_slot(self.body_id, widget);
        self.sync_id_slots();
        self.base.request_redraw();
    }

    /// The body page's id, when the slot was filled by [`Self::set_body_id`].
    pub fn body_id(&self) -> Option<ObjectId> {
        self.body_id
    }

    /// Assigns the body page for destination `index` by id.
    ///
    /// The page is stored at exactly `index`: the mapping is the document's, and clamping it
    /// against the item list would silently move every page to destination 0 in a document
    /// that declares its pages before its destinations.
    pub fn set_nav_body_id(&mut self, index: usize, widget: Option<ObjectId>) {
        // A destination index arrives from a document, so it is untrusted: growing the page
        // vector to an absurd index would abort on allocation. The ceiling is far above any
        // real navigation bar, and a refusal is reported rather than silently reshaped.
        const MAX_NAV_DESTINATIONS: usize = 1024;
        if index >= MAX_NAV_DESTINATIONS {
            log::warn!(
                "adaptive_scaffold: destination index {index} exceeds the supported \
                 {MAX_NAV_DESTINATIONS}; the declared page was not bound"
            );
            return;
        }
        if self.nav_body_ids.len() <= index {
            self.nav_body_ids.resize_with(index + 1, || None);
        }
        let previous = self.nav_body_ids[index];
        self.nav_body_ids[index] = self.adopt_id_slot(previous, widget);
        self.sync_id_slots();
        self.base.request_redraw();
    }

    /// The body page id assigned to `index`, when there is one.
    pub fn nav_body_id(&self, index: usize) -> Option<ObjectId> {
        self.nav_body_ids.get(index).copied().flatten()
    }

    /// Sets the floating action button slot by id.
    pub fn set_fab_id(&mut self, widget: Option<ObjectId>) {
        self.fab_id = self.adopt_id_slot(self.fab_id, widget);
        self.sync_id_slots();
        self.base.request_redraw();
    }

    /// Sets the leading drawer slot by id. The drawer stays closed until
    /// [`Self::open_drawer`] is called, the same contract as [`Self::set_drawer`].
    pub fn set_drawer_id(&mut self, widget: Option<ObjectId>) {
        self.drawer_id = self.adopt_id_slot(self.drawer_id, widget);
        if self.drawer_id.is_none() {
            self.drawer_open = false;
        }
        self.sync_id_slots();
        self.base.request_redraw();
    }

    /// Sets the trailing (end) drawer slot by id.
    pub fn set_end_drawer_id(&mut self, widget: Option<ObjectId>) {
        self.end_drawer_id = self.adopt_id_slot(self.end_drawer_id, widget);
        if self.end_drawer_id.is_none() {
            self.end_drawer_open = false;
        }
        self.sync_id_slots();
        self.base.request_redraw();
    }

    /// Sets the bottom-sheet slot by id.
    pub fn set_bottom_sheet_id(&mut self, widget: Option<ObjectId>) {
        self.bottom_sheet_id = self.adopt_id_slot(self.bottom_sheet_id, widget);
        self.sync_id_slots();
        self.base.request_redraw();
    }

    /// Sets the snackbar slot by id.
    pub fn set_snackbar_id(&mut self, widget: Option<ObjectId>) {
        self.snackbar_id = self.adopt_id_slot(self.snackbar_id, widget);
        self.sync_id_slots();
        self.base.request_redraw();
    }

    /// Links `next` under this control and unlinks the slot's previous occupant.
    ///
    /// The parent/child relation is written in both directions by
    /// [`BaseWidget::add_child_linked`], which also refuses a duplicate entry, so calling
    /// this after the runtime mounted the child under this control is the supported order.
    fn adopt_id_slot(
        &mut self,
        previous: Option<ObjectId>,
        next: Option<ObjectId>,
    ) -> Option<ObjectId> {
        if previous == next {
            return next;
        }
        if let Some(previous) = previous {
            self.base.remove_child_linked(previous);
            crate::set_widget_visible(previous, false);
        }
        if let Some(next) = next {
            self.base.add_child_linked(next);
        }
        next
    }

    /// Places and shows/hides every id slot.
    ///
    /// The bands come from the same private rect helpers the `Box` slot drawing uses, so the
    /// two content models cannot drift. Only the selected destination's page is visible, and
    /// a drawer is painted only while it is open or still sliding — the same conditions
    /// [`Self::draw`] applies to its own slots.
    pub(super) fn sync_id_slots(&self) {
        let content = self.content_rect();
        let show_id =
            self.nav_body_ids.get(self.selected_nav_index).copied().flatten().or(self.body_id);
        for (index, page) in self.nav_body_ids.iter().enumerate() {
            let Some(id) = *page else { continue };
            let shown = index == self.selected_nav_index && Some(id) == show_id;
            crate::set_widget_visible(id, shown);
            if shown {
                crate::set_widget_geometry(id, content.x, content.y, content.width, content.height);
            }
        }
        if let Some(id) = self.body_id {
            let shown = Some(id) == show_id;
            crate::set_widget_visible(id, shown);
            if shown {
                crate::set_widget_geometry(id, content.x, content.y, content.width, content.height);
            }
        }
        if let Some(id) = self.drawer_id {
            let progress = self.drawer_reveal.value().clamp(0.0, 1.0);
            let shown = self.drawer_open || progress > 0.0;
            crate::set_widget_visible(id, shown);
            if shown {
                let band = self.drawer_rect();
                crate::set_widget_geometry(id, band.x, band.y, band.width, band.height);
            }
        }
        if let Some(id) = self.end_drawer_id {
            let progress = self.end_drawer_reveal.value().clamp(0.0, 1.0);
            let shown = self.end_drawer_open || progress > 0.0;
            crate::set_widget_visible(id, shown);
            if shown {
                let band = self.end_drawer_rect();
                crate::set_widget_geometry(id, band.x, band.y, band.width, band.height);
            }
        }
        if let Some(id) = self.bottom_sheet_id {
            let band = self.bottom_sheet_rect();
            crate::set_widget_visible(id, true);
            crate::set_widget_geometry(id, band.x, band.y, band.width, band.height);
        }
        if let Some(id) = self.snackbar_id {
            let band = self.snackbar_rect();
            crate::set_widget_visible(id, true);
            crate::set_widget_geometry(id, band.x, band.y, band.width, band.height);
        }
        if let Some(id) = self.fab_id {
            match self.fab_rect() {
                Some(band) => {
                    crate::set_widget_visible(id, true);
                    crate::set_widget_geometry(id, band.x, band.y, band.width, band.height);
                }
                None => crate::set_widget_visible(id, false),
            }
        }
    }
}
