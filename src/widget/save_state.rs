// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Opt-in widget state persistence.
//!
//! # Why this is a trait, not reflection
//!
//! A control knows which of its fields describe *user state* and which describe its model. A
//! framework cannot recover that difference by reading structs: persisting everything would save
//! data the host owns, and guessing would produce a settings format coupled to private field
//! layout. A control therefore **declares** the state it is willing to hand over, together with a
//! format version, and the framework only moves the bytes.
//!
//! # Why the key comes from the caller
//!
//! [`crate::core::ObjectId`] is allocated at run time, so it cannot survive a restart. Deriving a
//! key from the tree shape would silently reattach state to a different control after a reorder.
//! [`collect_save_state`] and [`restore_save_state`] therefore take an explicit `key_of` resolver:
//! the host decides which mounted instance a key names, exactly as it decides the control's role
//! in its own layout.
//!
//! # Scope
//!
//! This is the **opt-in** half of the save-state decision: a host calls it when it wants session
//! restore. Nothing in the frame loop reads it, no control serialises itself automatically, and a
//! control that does not implement [`SaveState`] contributes nothing.

use crate::compat::{String, Vec};
use crate::core::ObjectId;
use crate::widget::runtime::{children_of, read_widget, with_widget_mut};

/// One widget's state as it crosses the persistence boundary.
///
/// `bytes` is the control's own format; `version` is what
/// [`SaveState::state_version`] reported when the bytes were produced. A reader compares the two
/// before restoring, so a format change cannot be misread as the old shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedState {
    /// The explicit, host-chosen key naming this component.
    pub key: String,
    /// The format version the producing control declared.
    pub version: u16,
    /// The control's serialised state.
    pub bytes: Vec<u8>,
}

impl SavedState {
    /// Builds one entry.
    pub fn new(key: impl Into<String>, version: u16, bytes: Vec<u8>) -> Self {
        Self { key: key.into(), version, bytes }
    }
}

/// A control that can hand its user-visible state across a restart.
///
/// # Contract
///
/// * [`Self::save_state`] must be self-contained: the bytes plus the version are everything
///   [`Self::restore_state`] needs.
/// * [`Self::restore_state`] must leave the control untouched and answer `false` for a malformed
///   or unsupported blob. It must never partially apply a state it rejects.
/// * Bumping [`Self::state_version`] is the supported way to change the format: an entry written
///   by an older version is then refused rather than misread.
///
/// Implementing the trait is the opt-in. The framework reaches it through
/// [`crate::widget::Widget::save_state_dyn`], which defaults to `None`.
pub trait SaveState {
    /// The version of the byte format [`Self::save_state`] produces.
    fn state_version(&self) -> u16;

    /// Serialises the control's user-visible state.
    fn save_state(&self) -> Vec<u8>;

    /// Restores a blob produced by [`Self::save_state`].
    ///
    /// Returns `false` when the blob is malformed or its version is unsupported, leaving the
    /// control unchanged.
    fn restore_state(&mut self, state: &[u8]) -> bool;
}

/// Walks `root` and collects the state of every mounted widget that opts in.
///
/// The walk is depth-first, visiting `root` before its descendants. `key_of` is the host's
/// explicit naming decision: returning `None` skips that widget, so an unnamed instance is not
/// persisted. Entries are sorted by key, which makes the produced list stable regardless of
/// traversal order.
///
/// A key declared by more than one widget is kept for the **first** widget in the walk and a
/// warning is logged for the rest; two components that answer to one key could not be restored
/// apart.
pub fn collect_save_state(
    root: ObjectId,
    key_of: impl Fn(ObjectId) -> Option<String>,
) -> Vec<SavedState> {
    let mut collected = Vec::new();
    walk(root, &mut |id| {
        let Some(key) = key_of(id) else {
            return;
        };
        if key.is_empty() {
            return;
        }
        if collected.iter().any(|entry: &SavedState| entry.key == key) {
            log::warn!(
                "save-state key '{key}' is declared by more than one widget; keeping the first"
            );
            return;
        }
        let saved = read_widget(id, |widget| {
            widget.save_state_dyn().map(|participant| SavedState {
                key,
                version: participant.state_version(),
                bytes: participant.save_state(),
            })
        })
        .flatten();
        if let Some(saved) = saved {
            collected.push(saved);
        }
    });
    collected.sort_by(|left, right| left.key.cmp(&right.key));
    collected
}

/// Restores every entry in `states` whose key names a mounted widget under `root`.
///
/// Returns how many widgets accepted their state. An entry is skipped when:
///
/// * no widget answers to its key,
/// * the widget does not implement [`SaveState`], or
/// * the entry's version differs from the widget's [`SaveState::state_version`], which is how a
///   format change refuses old bytes instead of misreading them.
///
/// A widget that answers `false` from [`SaveState::restore_state`] is also left untouched and is
/// not counted; a malformed payload therefore cannot make the returned count lie.
pub fn restore_save_state(
    root: ObjectId,
    states: &[SavedState],
    key_of: impl Fn(ObjectId) -> Option<String>,
) -> usize {
    let mut applied = 0;
    walk(root, &mut |id| {
        let Some(key) = key_of(id) else {
            return;
        };
        let Some(entry) = states.iter().find(|entry| entry.key == key) else {
            return;
        };
        let restored = with_widget_mut(id, |widget| {
            let Some(participant) = widget.save_state_dyn_mut() else {
                return false;
            };
            if participant.state_version() != entry.version {
                return false;
            }
            participant.restore_state(&entry.bytes)
        })
        .unwrap_or(false);
        if restored {
            applied += 1;
        }
    });
    applied
}

/// Depth-first pre-order walk over the mounted subtree rooted at `root`.
///
/// A visited list guards against a malformed parent link that would otherwise loop forever: an
/// id is visited once even if two parents claim it.
fn walk(root: ObjectId, visit: &mut impl FnMut(ObjectId)) {
    let mut stack = Vec::new();
    stack.push(root);
    let mut visited = Vec::new();
    while let Some(id) = stack.pop() {
        if visited.contains(&id) {
            continue;
        }
        visited.push(id);
        visit(id);
        let mut children = children_of(id);
        children.reverse();
        stack.extend(children);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::widget::container_widgets::groupbox::GroupBox;
    use crate::widget::container_widgets::splitter::Splitter;
    use crate::widget::container_widgets::tabwidget::TabWidget;
    use crate::widget::runtime::{register, unregister};
    use crate::widget::Widget;

    /// A root with one splitter and one tab widget, cleaned up by `Drop`.
    struct Fixture {
        root: ObjectId,
        splitter: ObjectId,
        tabs: ObjectId,
    }

    impl Fixture {
        fn new() -> Self {
            let root = register(Box::new(GroupBox::new(Rect::new(0, 0, 400, 300))))
                .expect("a test thread has a registry");
            let splitter = register(Box::new(Splitter::new(Rect::new(0, 0, 200, 200))))
                .expect("a test thread has a registry");
            let tabs = register(Box::new(TabWidget::new(Rect::new(0, 0, 200, 200))))
                .expect("a test thread has a registry");
            with_widget_mut(root, |parent| {
                parent.add_child(splitter);
                parent.add_child(tabs);
            });
            // Give both controls a real shape, or their state has nothing to describe.
            with_widget_mut(splitter, |widget| {
                let splitter = (widget as &mut dyn core::any::Any)
                    .downcast_mut::<Splitter>()
                    .expect("splitter");
                splitter.add_pane(0, 1);
                splitter.add_pane(0, 1);
                assert!(splitter.set_ratios(&[0.3, 0.7]));
            });
            with_widget_mut(tabs, |widget| {
                let tabs =
                    (widget as &mut dyn core::any::Any).downcast_mut::<TabWidget>().expect("tabs");
                tabs.add_tab("One".to_string(), None);
                tabs.add_tab("Two".to_string(), None);
            });
            Self { root, splitter, tabs }
        }

        fn key_of(&self) -> impl Fn(ObjectId) -> Option<String> {
            let splitter = self.splitter;
            let tabs = self.tabs;
            move |id| {
                if id == splitter {
                    Some("workspace.split".to_string())
                } else if id == tabs {
                    Some("workspace.tabs".to_string())
                } else {
                    None
                }
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            unregister(self.splitter);
            unregister(self.tabs);
            unregister(self.root);
        }
    }

    fn splitter_ratios(id: ObjectId) -> Vec<f32> {
        read_widget(id, |widget| {
            (widget as &dyn core::any::Any)
                .downcast_ref::<Splitter>()
                .map(|splitter| splitter.ratios())
        })
        .flatten()
        .expect("the id addresses a splitter")
    }

    fn tab_index(id: ObjectId) -> usize {
        read_widget(id, |widget| {
            (widget as &dyn core::any::Any)
                .downcast_ref::<TabWidget>()
                .map(|tabs| tabs.selected_index().unwrap_or(0))
        })
        .flatten()
        .expect("the id addresses a tab widget")
    }

    #[test]
    fn collect_uses_the_host_keys_and_sorts_them() {
        let fixture = Fixture::new();
        let states = collect_save_state(fixture.root, fixture.key_of());
        assert_eq!(states.len(), 2, "one entry per opted-in widget");
        assert_eq!(states[0].key, "workspace.split", "sorted by key");
        assert_eq!(states[1].key, "workspace.tabs");
        assert!(states.iter().all(|entry| entry.version == 1));
        assert!(!states[0].bytes.is_empty());
    }

    #[test]
    fn a_widget_without_a_key_is_not_collected() {
        let fixture = Fixture::new();
        let states = collect_save_state(fixture.root, |_| None);
        assert!(states.is_empty(), "an unnamed instance must not be persisted");
        let states = collect_save_state(fixture.root, |_| Some(String::new()));
        assert!(states.is_empty(), "an empty key is the same refusal");
    }

    #[test]
    fn restore_applies_matching_keys() {
        let fixture = Fixture::new();
        let states = collect_save_state(fixture.root, fixture.key_of());
        assert_eq!(states.len(), 2);

        // Move both controls away from the snapshot, then restore it.
        with_widget_mut(fixture.splitter, |widget| {
            let splitter =
                (widget as &mut dyn core::any::Any).downcast_mut::<Splitter>().expect("splitter");
            assert!(splitter.set_ratios(&[0.9, 0.1]));
        });
        with_widget_mut(fixture.tabs, |widget| {
            let tabs =
                (widget as &mut dyn core::any::Any).downcast_mut::<TabWidget>().expect("tabs");
            tabs.set_current_index(1);
        });
        assert_eq!(splitter_ratios(fixture.splitter), vec![0.9, 0.1]);
        assert_eq!(tab_index(fixture.tabs), 1);

        let applied = restore_save_state(fixture.root, &states, fixture.key_of());
        assert_eq!(applied, 2);
        assert_eq!(splitter_ratios(fixture.splitter), vec![0.3, 0.7]);
        assert_eq!(tab_index(fixture.tabs), 0, "the saved selection comes back");
    }

    #[test]
    fn restore_skips_a_version_mismatch() {
        let fixture = Fixture::new();
        let mut states = collect_save_state(fixture.root, fixture.key_of());
        for entry in &mut states {
            entry.version = entry.version.wrapping_add(1);
        }
        let applied = restore_save_state(fixture.root, &states, fixture.key_of());
        assert_eq!(applied, 0, "an unknown version must be refused, not misread");
    }

    #[test]
    fn restore_rejects_a_malformed_payload_without_counting_it() {
        let fixture = Fixture::new();
        let states = vec![SavedState::new("workspace.split", 1, vec![0xff, 0x00])];
        let applied = restore_save_state(fixture.root, &states, fixture.key_of());
        assert_eq!(applied, 0);
        assert_eq!(splitter_ratios(fixture.splitter), vec![0.3, 0.7], "nothing was applied");
    }
}
