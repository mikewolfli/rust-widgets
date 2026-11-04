// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The carousel's page model: one page's content slot and the trait it is drawn through.
//!
//! Split out of `carousel.rs`, which owns the control itself (paging, gestures, autoplay,
//! indicator) and was pushed past the source-structure limit by the declared-child slot
//! (principle #9). The seam is the model/control boundary: this file describes what a page
//! *holds*, `carousel.rs` decides when it is shown and painted.
//!
//! Both types are re-exported through `carousel.rs`, so `Carousel::add_page`,
//! `Carousel::set_page_content` and the public paths `widget::CarouselPage` /
//! `widget::WidgetAndDraw` are unchanged.

use crate::core::Color;
use crate::object::ObjectId;
use crate::render::RenderContext;
use crate::widget::{Draw, Widget};

/// A page's content: a control that can be drawn into the carousel's content area.
///
/// This trait exists because [`Draw`] is not dyn-compatible — a `Box<dyn Widget>`
/// cannot be drawn through it. The blanket implementation below means any type that
/// is both a [`Widget`] and [`Draw`] qualifies, so a caller mounts a control with
/// `Box::new(my_widget)` and nothing else.
///
/// It is declared beside the page model that consumes it, rather than in a shared
/// module: the only thing that draws through it is the carousel's page slot, so a
/// second declaration site would be a second thing to keep in step for no benefit
/// (BLUE18 rule #28).
pub trait WidgetAndDraw: Widget {
    /// Draw this widget using the provided render context.
    fn draw_widget(&mut self, context: &mut RenderContext);
}

impl<T: Widget + Draw> WidgetAndDraw for T {
    fn draw_widget(&mut self, context: &mut RenderContext) {
        self.draw(context);
    }
}

/// Data for a single carousel page built from a title and a background color.
///
/// This is the lightweight spelling of a page: a caller that only wants a colored
/// slide with a caption uses `Carousel::add_page`. A caller that wants a real
/// control inside uses `Carousel::set_page_content` instead, and the two can be
/// mixed — a page's slot is either a title card or a mounted widget.
pub struct CarouselPage {
    /// Display title shown centered on the page.
    pub title: String,
    /// Background color of the page.
    pub color: Color,
    /// Optional child control drawn inside this page's content area.
    ///
    /// `None` means "draw the title card", which is what `add_page` produces. Visible to the
    /// carousel (its only writer), which stores an owned control here through
    /// `Carousel::set_page_content`.
    pub(super) content: Option<Box<dyn WidgetAndDraw>>,
    /// The page's child when the runtime already owns it (issue-20261104 DM-11).
    ///
    /// A declared child cannot be handed over as a `Box`: the loader mounts it and the registry
    /// keeps it. This slot records the id instead; the carousel writes its geometry and
    /// visibility and the tree painter draws it, exactly as the other containers' id slots do. An
    /// id slot wins over a `Box` slot on the same page.
    content_id: Option<ObjectId>,
}

impl CarouselPage {
    /// Creates a title-only page.
    pub fn new(title: impl Into<String>, color: Color) -> Self {
        Self { title: title.into(), color, content: None, content_id: None }
    }

    /// Returns whether this page has a mounted child control.
    pub fn has_content(&self) -> bool {
        self.content.is_some() || self.content_id.is_some()
    }

    /// Fills this page's content area with a control the runtime already mounted.
    pub fn set_content_id(&mut self, content: Option<ObjectId>) {
        self.content_id = content;
    }

    /// The page's declared content, when it was filled by [`Self::set_content_id`].
    pub fn content_id(&self) -> Option<ObjectId> {
        self.content_id
    }

    /// Borrows the page's child control, if one was set.
    pub fn content(&self) -> Option<&dyn WidgetAndDraw> {
        self.content.as_deref().map(|content| content as &dyn WidgetAndDraw)
    }

    /// Mutably borrows the page's child control, if one was set.
    pub fn content_mut(&mut self) -> Option<&mut dyn WidgetAndDraw> {
        match self.content.as_deref_mut() {
            Some(content) => Some(content),
            None => None,
        }
    }
}
