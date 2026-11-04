// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tab-widget handle operations (RW-4).
//!
//! A tab widget's pages are child ids, not layout tracks: one page is selected and drawn in the
//! content rect, so the operations here are content/selection operations rather than `Layout`
//! registrations.

use super::*;
use crate::core::{ObjectId, Point};

/// # Tab-widget specific operations
///
/// A tab widget's pages are child ids, not layout tracks: one page is selected and drawn in
/// the content rect, so the operations here are content/selection operations rather than
/// `Layout` registrations. Every method answers honestly when the id is not a mounted
/// `TabWidget`.
impl TabWidgetHandle {
    /// Appends a tab titled `title`, hosting `page` (or no page).
    ///
    /// Returns the new index, or `None` when the id is not a mounted tab widget.
    pub fn add_tab(&self, title: &str, page: Option<ObjectId>) -> Option<usize> {
        let added = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.add_tab(title.to_owned(), page))
        })
        .flatten();
        if added.is_some() {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        added
    }

    /// Removes the tab at `index`.
    ///
    /// Returns `false` when the id is not a tab widget or the index is out of range.
    pub fn remove_tab(&self, index: usize) -> bool {
        let removed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| {
                    let valid = index < tabs.count();
                    if valid {
                        tabs.remove_tab(index);
                    }
                    valid
                })
        })
        .flatten()
        .unwrap_or(false);
        if removed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        removed
    }

    /// Returns the number of tabs, or `0` when the id is not a tab widget.
    pub fn tab_count(&self) -> usize {
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.count())
                .unwrap_or(0)
        })
        .unwrap_or(0)
    }

    /// Returns the selected tab index, or `None` when the id is not a tab widget.
    pub fn current_index(&self) -> Option<usize> {
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::TabWidget>(widget)
                .and_then(|tabs| tabs.selected_index())
        })
        .flatten()
    }

    /// Selects the tab at `index`.
    ///
    /// Returns `false` when the id is not a tab widget or the index is out of range; an
    /// out-of-range index is not silently clamped to the last tab.
    pub fn set_current_index(&self, index: usize) -> bool {
        let selected = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| {
                    let valid = index < tabs.count();
                    if valid {
                        tabs.set_current_index(index);
                    }
                    valid
                })
        })
        .flatten()
        .unwrap_or(false);
        if selected {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        selected
    }

    /// The tab index under `pos`, using the same geometry the strip paints.
    ///
    /// A context-menu handler calls this to bind its menu to the tab the user right-clicked
    /// rather than the active one (feedback C-9). `None` over the content page, past the last tab,
    /// or for an id that is not a mounted tab widget.
    pub fn tab_index_at(&self, pos: Point) -> Option<usize> {
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::TabWidget>(widget)
                .and_then(|tabs| tabs.tab_index_at(pos))
        })
        .flatten()
    }

    /// Subscribes to a context-menu request on the tab strip, with the pointer position.
    ///
    /// The tab under the pointer is selected before this fires, so [`Self::current_index`] and the
    /// menu describe the same page (feedback C-9). Content-page requests are not delivered.
    pub fn on_context_menu_requested<F>(&self, f: F)
    where
        F: FnMut(Point) + Send + 'static,
    {
        let id = self.raw_id();
        let held: std::sync::Arc<std::sync::Mutex<F>> =
            std::sync::Arc::new(std::sync::Mutex::new(f));
        let connected = crate::widget::runtime::with_widget_mut(id, |widget| {
            let held = std::sync::Arc::clone(&held);
            (widget as &mut dyn core::any::Any).downcast_mut::<crate::widget::TabWidget>().map(
                |tabs| {
                    tabs.context_menu_requested.connect_ref(move |pos: &Point| {
                        if let Ok(mut callback) = held.lock() {
                            callback(*pos);
                        } else {
                            log::warn!(
                                "the tab-strip context-menu callback for widget {id} could not be \
                                 locked; skipping this request rather than panicking"
                            );
                        }
                    })
                },
            )
        })
        .flatten();
        match connected {
            Some(handle) => record_signal_connection(id, SignalKind::Triggered, handle),
            None => {
                log::warn!(
                    "on_context_menu_requested ignored for id={id}: it is not a live tab widget, \
                     so the callback will never run."
                );
                record_unwired_binding(id);
            }
        }
    }

    /// Replaces the title of the tab at `index`.
    pub fn set_tab_text(&self, index: usize, text: &str) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| {
                    let valid = index < tabs.count();
                    if valid {
                        tabs.set_tab_text(index, text.to_owned());
                    }
                    valid
                })
        })
        .flatten()
        .unwrap_or(false);
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Shows or hides close buttons on the tabs.
    pub fn set_closable(&self, closable: bool) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_closable(closable))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Enables or disables drag-to-reorder on the tab strip.
    pub fn set_movable(&self, movable: bool) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_movable(movable))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Moves the tab strip to one of the four sides.
    pub fn set_tab_position(&self, position: crate::app::TabPosition) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_tab_position(position))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Selects the tab corner shape.
    pub fn set_tab_shape(&self, shape: crate::app::TabShape) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_tab_shape(shape))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Inserts a tab at `index`, hosting `page` (or no page) (RW-4).
    ///
    /// The typed entry clamps an index past the end to an append, which is the drag path's
    /// intent; this forward keeps that rule. Returns `false` when the id is not a mounted
    /// tab widget.
    pub fn insert_tab(&self, index: usize, title: &str, page: Option<ObjectId>) -> bool {
        let inserted = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.insert_tab(index, title.to_owned(), page))
        })
        .flatten()
        .is_some();
        if inserted {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        inserted
    }

    /// Moves the tab at `from` to `to` (RW-4).
    ///
    /// `to` is clamped by the control; an out-of-range `from` is refused. Returns whether a
    /// move happened, so a dropped write is not reported as success.
    pub fn move_tab(&self, from: usize, to: usize) -> bool {
        let moved = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.move_tab(from, to))
        })
        .flatten()
        .unwrap_or(false);
        if moved {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        moved
    }

    /// Assigns typed icon content to the tab at `index` (RW-4).
    ///
    /// Returns `false` for an out-of-range index or an id that is not a mounted tab widget.
    pub fn set_item_icon_source(
        &self,
        index: usize,
        source: Option<crate::widget::IconSource>,
    ) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_item_icon_source(index, source).is_ok())
        })
        .flatten()
        .unwrap_or(false);
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Sets the tooltip of the tab at `index` (RW-4).
    pub fn set_tab_tooltip(&self, index: usize, text: &str) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_tab_tooltip(index, text.to_owned()))
        })
        .flatten()
        .unwrap_or(false);
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Shows or hides the tab at `index` (RW-4).
    pub fn set_tab_visible(&self, index: usize, visible: bool) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_tab_visible(index, visible))
        })
        .flatten()
        .unwrap_or(false);
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Enables or disables the tab at `index` (RW-4).
    pub fn set_tab_enabled(&self, index: usize, enabled: bool) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_tab_enabled(index, enabled))
        })
        .flatten()
        .unwrap_or(false);
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Serialises the selected tab (RW-4).
    ///
    /// `None` when the id is not a mounted tab widget. The blob is the control's own format;
    /// pass it back to [`Self::restore_state`].
    pub fn save_state(&self) -> Option<crate::compat::Vec<u8>> {
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.save_state())
        })
        .flatten()
    }

    /// Restores a blob produced by [`Self::save_state`] (RW-4).
    ///
    /// Returns `false` when the id is not a mounted tab widget or the control refused the
    /// blob; a refused restore leaves the selection untouched.
    pub fn restore_state(&self, state: &[u8]) -> bool {
        let restored = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.restore_state(state))
        })
        .flatten()
        .unwrap_or(false);
        if restored {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        restored
    }
}
