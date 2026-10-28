// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The web model **and** the web controls, in a package of their own.
//!
//! # Why this is a package of its own
//!
//! These modules model a browser — a page's URL, title and load lifecycle, its history, its
//! cookie jar and privacy rules, its plugin registry, and the script engine that evaluates its
//! JavaScript — and they carry the controls a host mounts to show one (`WebEngineView`,
//! `WebFindBar`, `WebHistoryPanel`). Before the move recorded in `docs/log/log-20261028-1.md`
//! §11 they were two trees inside `rust_widgets` (`src/web/` and `src/widget/web_widgets/`); they
//! are one package now, so the classification follows the feature rather than the crate boundary.
//!
//! # Dependency direction
//!
//! The dependency runs **one way**: this package depends on `rust_widgets` for `Widget`,
//! `BaseWidget`, `RenderContext`, the signal types and the platform query. The parent never
//! depends on this package — Cargo rejects a package cycle, and a host that wants the web controls
//! installs them explicitly:
//!
//! ```ignore
//! rust_widgets_webview::install(&mut factory)?;
//! ```
//!
//! # What is here
//!
//! * [`js_engine`] — an engine-neutral JavaScript subset ([`JsValue`], [`JsContext`]), the
//!   dependency-free [`SimpleJsEngine`], and [`BoaJsEngine`] when `js-engine` is enabled.
//! * [`history`] / [`navigation`] — [`HistoryEntry`], [`BrowserHistory`], [`SessionHistory`],
//!   [`NavigationHistory`], [`LoadStatus`], [`WebSettings`].
//! * [`privacy`] — [`PrivacySettings`], [`CookieJar`], [`TrackingProtection`].
//! * [`plugins`] — [`PluginManager`] and the [`Plugin`] trait.
//! * [`web_engine`] / [`web_view`] / [`web_history_panel`] / [`find_bar`] — the controls.
//! * [`web_core`] — the shared core behind the two duplicate wrappers,
//!   [`WebEngineViewEnhanced`] and [`WebViewEnhanced`], which are kept so the names the old
//!   `rust_widgets::web` module published still resolve.

pub mod capability;
pub mod find_bar;
pub mod history;
pub mod js_engine;
pub mod navigation;
pub mod plugins;
pub mod privacy;
pub mod web_core;
pub mod web_engine;
/// The duplicate `WebEngineView` wrapper kept for API parity (see [`web_core`]).
pub mod web_engine_enhanced;
pub mod web_history_panel;
pub mod web_view;
/// The duplicate `WebView` wrapper kept for API parity (see [`web_core`]).
pub mod web_view_enhanced;

pub use capability::{
    install, web_find_bar_capability, web_history_panel_capability, web_view_capability,
};
pub use find_bar::WebFindBar;
pub use history::*;
pub use js_engine::*;
pub use navigation::*;
pub use plugins::*;
pub use privacy::*;
pub use web_core::WebViewCore;
pub use web_engine::{
    Cookie, CookieSameSite, HistoryEntry, WebEngineContextMenuRequest, WebEngineCookieStore,
    WebEngineDownloadItem, WebEngineFindTextResult, WebEngineNotification, WebEnginePage,
    WebEngineScriptDialog, WebEngineSettings, WebEngineView, WebEngineWebChannel,
    DEFAULT_USER_AGENT,
};
pub use web_engine_enhanced::WebEngineViewEnhanced;
pub use web_history_panel::{WebHistoryPanel, WebHistoryRecord};
pub use web_view::WebView;
pub use web_view_enhanced::WebViewEnhanced;

/// Implements `Default` in terms of a type's own `new()`.
///
/// The parent crate's `draw_bridge` macro has the same shape; it is duplicated rather than shared
/// because a macro cannot cross the package boundary without making this crate's public surface
/// depend on the parent's internals.
#[macro_export]
macro_rules! impl_default_via_new {
    ($type:ty) => {
        impl ::core::default::Default for $type {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

#[cfg(test)]
mod reexport_tests {
    //! The names the old `rust_widgets::web` module published still resolve from this crate's root.
    //!
    //! These are the tests that lived in `src/web/mod.rs` before the move. They are re-exports, so
    //! nothing else in the crate calls them: without a test that names each one, a deleted
    //! `pub use` would compile and the name would silently disappear from the public API.
    use super::*;
    use rust_widgets::core::Rect;

    #[test]
    fn the_model_types_are_reachable_from_the_root() {
        let entry = NavigationEntry::default();
        assert_eq!(entry.url, "about:blank");

        let history = NavigationHistory::new(10);
        assert!(history.is_empty());

        let settings = WebSettings::default();
        assert!(settings.javascript_enabled);

        let security = SecuritySettings::default();
        assert!(security.block_popups);
    }

    #[test]
    fn the_core_is_reachable_from_the_root() {
        let core = WebViewCore::new(
            rust_widgets::widget::WidgetKind::WebEngineView,
            Rect::new(0, 0, 800, 600),
            "test",
            "about:blank",
        );
        assert_eq!(core.url(), "about:blank");
    }

    #[test]
    fn the_two_duplicate_wrappers_are_reachable_from_the_root() {
        let engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert_eq!(engine.url(), "");

        let view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert_eq!(view.url(), "about:blank");
    }

    #[test]
    fn the_script_engine_is_reachable_from_the_root() {
        let mut engine = SimpleJsEngine::new();
        let mut ctx = JsContext::new();
        let result = engine.evaluate("42", &mut ctx).unwrap();
        assert_eq!(result, JsValue::Number(42.0));
        engine.evaluate("var x = 10;", &mut ctx).unwrap();
        assert_eq!(engine.evaluate("x", &mut ctx).unwrap(), JsValue::Number(10.0));
    }

    #[test]
    fn the_plugin_and_privacy_types_are_reachable_from_the_root() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        assert_eq!(id, 1);
        assert!(!mgr.list().is_empty());

        let mut jar = CookieJar::new();
        // Qualified because the widget module publishes its own `Cookie` (a history/HTTP cookie
        // record); the jar takes the privacy one.
        jar.add(crate::privacy::Cookie::new(
            "s".to_string(),
            "v".to_string(),
            "example.com".to_string(),
        ));
        assert_eq!(jar.len(), 1);

        let data = BrowsingData::all();
        assert!(data.history && data.cookies);

        let mut tp = TrackingProtection::new(PrivacySettings::strict());
        assert!(tp.check_tracking(TrackingType::Fingerprinting, "t.com", "https://t.com"));
    }
}
