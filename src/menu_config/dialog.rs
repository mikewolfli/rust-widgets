// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{ConfigLoadError, ConfigPersistence, MenuConfig, PerformanceLevel};
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::metrics::{dimensions, effective_font};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::io;
/// Menu configuration dialog for user preferences.
pub struct MenuConfigDialog {
    config: MenuConfig,
    persistence: ConfigPersistence,
}
impl MenuConfigDialog {
    /// Creates a new configuration dialog.
    pub fn new(config: MenuConfig) -> Self {
        Self { config, persistence: ConfigPersistence::new() }
    }
    /// Creates a dialog with custom persistence.
    pub fn with_persistence(config: MenuConfig, persistence: ConfigPersistence) -> Self {
        Self { config, persistence }
    }
    /// Gets the current configuration.
    pub fn config(&self) -> &MenuConfig {
        &self.config
    }
    /// Gets mutable configuration.
    pub fn config_mut(&mut self) -> &mut MenuConfig {
        &mut self.config
    }
    /// Whether the adaptive configuration is applied.
    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled()
    }
    /// Toggles the adaptive configuration on or off.
    pub fn toggle_enabled(&mut self) {
        let new_value = !self.config.is_enabled();
        self.config.set_enabled(new_value);
    }
    /// Toggles animations on/off.
    pub fn toggle_animations(&mut self) {
        let new_value = !self.config.animations_enabled();
        self.config.set_animations_enabled(new_value);
    }
    /// Toggles transparency on/off.
    pub fn toggle_transparency(&mut self) {
        let new_value = !self.config.transparency_enabled();
        self.config.set_transparency_enabled(new_value);
    }
    /// Toggles shadows on/off.
    pub fn toggle_shadows(&mut self) {
        let new_value = !self.config.shadows_enabled();
        self.config.set_shadows_enabled(new_value);
    }
    /// Toggles blur on/off.
    pub fn toggle_blur(&mut self) {
        let new_value = !self.config.blur_enabled();
        self.config.set_blur_enabled(new_value);
    }
    /// Increases animation speed.
    pub fn increase_animation_speed(&mut self) {
        let current = self.config.animation_speed();
        let new_speed = (current + 0.1).min(3.0);
        self.config.set_animation_speed(new_speed);
    }
    /// Decreases animation speed.
    pub fn decrease_animation_speed(&mut self) {
        let current = self.config.animation_speed();
        let new_speed = (current - 0.1).max(0.1);
        self.config.set_animation_speed(new_speed);
    }
    /// Increases max visible items, saturating at the type's maximum rather than
    /// wrapping back to a small value.
    pub fn increase_max_items(&mut self) {
        let current = self.config.max_visible_items();
        self.config.set_max_visible_items(current.saturating_add(5));
    }
    /// Decreases max visible items.
    pub fn decrease_max_items(&mut self) {
        let current = self.config.max_visible_items();
        if current > 5 {
            self.config.set_max_visible_items(current - 5);
        }
    }
    /// Resets all settings to hardware defaults.
    pub fn reset_to_defaults(&mut self) {
        self.config.reset_to_defaults();
    }
    /// Saves the current configuration.
    pub fn save(&self) -> io::Result<()> {
        self.persistence.save(&self.config)
    }
    /// Loads configuration from disk, restoring the hardware baseline first so a
    /// field removed from the file falls back to its default rather than keeping a
    /// stale reversed preference.
    ///
    /// A rejected load leaves `self.config` untouched, so a parse failure cannot
    /// corrupt the previous configuration.
    pub fn load(&mut self) -> Result<(), ConfigLoadError> {
        let overrides = self.persistence.load()?;
        self.config.reload_overrides(overrides);
        Ok(())
    }
    /// Returns a summary of current settings for display.
    pub fn settings_summary(&self) -> String {
        self.config.settings_summary()
    }
    /// Returns true if any settings have been overridden.
    pub fn has_overrides(&self) -> bool {
        self.config.has_user_overrides()
    }
    /// Returns the hardware-detected performance level.
    pub fn hardware_performance_level(&self) -> PerformanceLevel {
        self.config.hardware_caps().performance_level
    }
    /// Returns a description of the detected GPU.
    ///
    /// The memory figure is only printed when it was actually measured. This crate
    /// cannot query VRAM portably (wgpu exposes no size, the platform backends report
    /// system RAM), so on most hosts the honest description is the GPU type alone —
    /// the previous wording printed a defaulted `512` MB as if it had been detected.
    pub fn gpu_description(&self) -> String {
        let caps = self.config.hardware_caps();
        match (caps.gpu_memory_mb, caps.gpu_memory_is_measured) {
            (Some(mb), true) => format!("{} ({mb} MB)", caps.gpu_type.description()),
            _ => format!("{} (memory not reportable)", caps.gpu_type.description()),
        }
    }

    /// Returns a description of the detected system memory.
    ///
    /// The same rule [`Self::gpu_description`] follows: a figure is printed only when a
    /// backend actually measured it. When nothing could report physical memory the
    /// description says so, and names the conservative assumption the *scoring* model
    /// uses instead — clearly labelled as an assumption, so it cannot be read as a probe
    /// result.
    pub fn system_memory_description(&self) -> String {
        match self.config.hardware_caps().system_ram_mb {
            Some(mb) => format!("{mb} MB"),
            None => format!(
                "not reportable on this backend (scoring assumes {} MB)",
                MenuConfig::assumed_system_memory()
            ),
        }
    }
}

/// One switch row in a [`MenuConfigPanel`].
struct ConfigRow {
    label: &'static str,
    read: fn(&MenuConfig) -> bool,
    toggle: fn(&mut MenuConfig),
}

/// The settings panel's rows, in display order.
const CONFIG_ROWS: &[ConfigRow] = &[
    ConfigRow {
        label: "Adaptive configuration",
        read: MenuConfig::is_enabled,
        toggle: |config| config.set_enabled(!config.is_enabled()),
    },
    ConfigRow {
        label: "Animations",
        read: MenuConfig::animations_enabled,
        toggle: |config| config.set_animations_enabled(!config.animations_enabled()),
    },
    ConfigRow {
        label: "Transparency",
        read: MenuConfig::transparency_enabled,
        toggle: |config| {
            let next = !config.transparency_enabled();
            config.set_transparency_enabled(next);
        },
    },
    ConfigRow {
        label: "Shadows",
        read: MenuConfig::shadows_enabled,
        toggle: |config| {
            let next = !config.shadows_enabled();
            config.set_shadows_enabled(next);
        },
    },
    ConfigRow {
        label: "Blur",
        read: MenuConfig::blur_enabled,
        toggle: |config| {
            let next = !config.blur_enabled();
            config.set_blur_enabled(next);
        },
    },
    ConfigRow {
        label: "Hardware acceleration",
        read: MenuConfig::hardware_acceleration,
        toggle: |config| {
            let next = !config.hardware_acceleration();
            config.set_hardware_acceleration(next);
        },
    },
    ConfigRow {
        label: "Text measurement cache",
        read: MenuConfig::caching_enabled,
        toggle: |config| {
            let next = !config.caching_enabled();
            config.set_caching_enabled(next);
        },
    },
];

/// A settings panel for the adaptive menu configuration.
///
/// Unlike [`MenuConfigDialog`], which is a headless facade over the configuration and its
/// file, this is a real widget: it paints a labelled switch per policy field, hit-tests
/// its rows, and reports every change so a host can apply and persist it. The name clash
/// the previous scan reported — a "dialog" that was not a widget — is resolved by adding
/// the widget rather than by renaming the facade, so both the headless and the drawn
/// entry points exist.
pub struct MenuConfigPanel {
    base: BaseWidget,
    config: MenuConfig,
    hovered_row: Option<usize>,
    /// Emitted after any row changes, with no payload. The panel has already applied the
    /// change to its own [`MenuConfig`]; a listener reads the configuration and saves it.
    pub changed: GenericSignal,
}

impl MenuConfigPanel {
    /// Creates the panel for `config` in `geometry`.
    pub fn new(config: MenuConfig, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Menu, geometry, "MenuConfigPanel"),
            config,
            hovered_row: None,
            changed: GenericSignal::new(),
        }
    }

    /// The configuration the panel edits.
    pub fn config(&self) -> &MenuConfig {
        &self.config
    }

    /// The configuration the panel edits, mutably.
    pub fn config_mut(&mut self) -> &mut MenuConfig {
        &mut self.config
    }

    /// The number of switch rows.
    pub fn row_count(&self) -> usize {
        CONFIG_ROWS.len()
    }

    /// The label of row `index`, or `None` when out of range.
    pub fn row_label(&self, index: usize) -> Option<&'static str> {
        CONFIG_ROWS.get(index).map(|row| row.label)
    }

    /// Whether the switch at `index` is on, or `None` when out of range.
    pub fn row_value(&self, index: usize) -> Option<bool> {
        CONFIG_ROWS.get(index).map(|row| (row.read)(&self.config))
    }

    /// Applies row `index`'s toggle and announces the change.
    ///
    /// Returns `false` for an out-of-range index, which is how a caller can tell a
    /// mis-numbered row from a switch that refused the write.
    pub fn toggle_row(&mut self, index: usize) -> bool {
        let Some(row) = CONFIG_ROWS.get(index) else {
            return false;
        };
        (row.toggle)(&mut self.config);
        self.base.request_redraw();
        self.changed.emit();
        true
    }

    fn row_height() -> u32 {
        dimensions::MENU_ROW_PADDING_H.max(8) * 2 + 8
    }

    /// The box of row `index`.
    pub fn row_rect(&self, index: usize) -> Rect {
        let geom = self.geometry();
        let height = Self::row_height();
        Rect::new(
            geom.x,
            geom.y + dimensions::MENU_POPUP_PADDING as i32 + index as i32 * height as i32,
            geom.width,
            height,
        )
    }

    fn row_at(&self, pos: Point) -> Option<usize> {
        (0..CONFIG_ROWS.len()).find(|index| self.row_rect(*index).contains_point(pos))
    }
}

impl Widget for MenuConfigPanel {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> crate::core::Size {
        let height =
            dimensions::MENU_POPUP_PADDING * 2 + CONFIG_ROWS.len() as u32 * Self::row_height();
        crate::core::Size::new(260, height)
    }
    impl_draw_bridge!();
}

impl EventHandler for MenuConfigPanel {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let next = self.row_at(*pos);
                if next != self.hovered_row {
                    self.hovered_row = next;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_row.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.row_at(*pos) {
                    self.toggle_row(index);
                }
            }
            _ => {}
        }
    }
}

impl Draw for MenuConfigPanel {
    fn draw(&mut self, context: &mut RenderContext) {
        let geom = self.geometry();
        let style = self.base.style();
        let surface = style.background_color.unwrap_or(Color::rgb(245, 245, 245));
        let ink = style.text_color.unwrap_or_else(|| surface.contrast_color());
        let accent = crate::style::palette_shared()
            .map(|colors| colors.primary)
            .unwrap_or(Color::rgb(0, 120, 215));
        context.fill_rounded_rect(geom, 4, surface);
        context.draw_rounded_rect_stroke(geom, 4, surface.blend(&ink, 0.3), 1);
        let font = effective_font(style);
        for (index, spec) in CONFIG_ROWS.iter().enumerate() {
            let row = self.row_rect(index);
            if self.hovered_row == Some(index) {
                context.fill_rect(row, surface.blend(&accent, 0.14));
            }
            let line = context.text_line(row, font);
            context.draw_text_fitted_cow(
                Rect::new(row.x + 10, line.y, row.width.saturating_sub(60), line.height.max(1)),
                spec.label,
                font,
                ink,
                HorizontalAlignment::Left,
            );
            let on = (spec.read)(&self.config);
            context.draw_text_fitted_cow(
                Rect::new(row.x + row.width as i32 - 52, line.y, 42, line.height.max(1)),
                if on { "On" } else { "Off" },
                font,
                if on { accent } else { ink.blend(&surface, 0.4) },
                HorizontalAlignment::Right,
            );
        }
    }
}
