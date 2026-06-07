//! Web capabilities module — provides web view, web engine, navigation, history,
//! JavaScript bridge, privacy controls, and plugin management.
mod history;
mod js_engine;
mod navigation;
mod plugins;
mod privacy;
mod web_core;
mod web_engine;
mod web_view;
pub use history::*;
pub use js_engine::*;
pub use navigation::*;
pub use plugins::*;
pub use privacy::*;
pub use web_engine::*;
pub use web_view::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn test_mod_web_core_types_accessible() {
        let core = web_core::WebViewCore::new(
            crate::widget::WidgetKind::WebView,
            Rect::new(0, 0, 800, 600),
            "test",
            "about:blank",
        );
        assert_eq!(core.url(), "about:blank");
    }

    #[test]
    fn test_mod_navigation_types_accessible() {
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
    fn test_mod_history_types_accessible() {
        let mut bh = BrowserHistory::new();
        bh.add_entry("https://example.com".to_string(), "Example".to_string());
        assert_eq!(bh.len(), 1);

        let mut sh = SessionHistory::new(10);
        sh.navigate("https://example.com".to_string());
        assert!(sh.current().is_some());

        let entry = HistoryEntry::new("https://example.com".to_string(), "Ex".to_string());
        assert_eq!(entry.visit_count, 1);
    }

    #[test]
    fn test_mod_js_engine_types_accessible() {
        let val = JsValue::Number(42.0);
        assert_eq!(val.to_string(), "42");

        let mut engine = SimpleJsEngine::new();
        let mut ctx = JsContext::new();
        // SimpleJsEngine does not evaluate arithmetic expressions;
        // test with literal values and variable assignment instead.
        let result = engine.evaluate("42", &mut ctx).unwrap();
        assert_eq!(result, JsValue::Number(42.0));
        engine.evaluate("var x = 10;", &mut ctx).unwrap();
        let x_val = engine.evaluate("x", &mut ctx).unwrap();
        assert_eq!(x_val, JsValue::Number(10.0));
    }

    #[test]
    fn test_mod_plugins_types_accessible() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        assert_eq!(id, 1);
        assert!(!mgr.list().is_empty());

        let err = PluginError::new("fail".to_string());
        assert!(err.message.contains("fail"));
    }

    #[test]
    fn test_mod_privacy_types_accessible() {
        let settings = PrivacySettings::new();
        assert!(settings.do_not_track);

        let mut jar = CookieJar::new();
        let cookie = Cookie::new("s".to_string(), "v".to_string(), "example.com".to_string());
        jar.add(cookie);
        assert_eq!(jar.len(), 1);

        let data = BrowsingData::all();
        assert!(data.history && data.cookies);

        let mut tp = TrackingProtection::new(PrivacySettings::strict());
        let blocked = tp.check_tracking(TrackingType::Fingerprinting, "t.com", "https://t.com");
        assert!(blocked);
    }

    #[test]
    fn test_mod_web_engine_enhanced_accessible() {
        let engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert_eq!(engine.url(), "");
    }

    #[test]
    fn test_mod_web_view_enhanced_accessible() {
        let view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
        assert_eq!(view.url(), "about:blank");
    }

    #[test]
    fn test_mod_load_status_accessible() {
        assert_eq!(LoadStatus::NotStarted as u8, 0);
        assert_eq!(LoadStatus::Loading as u8, 1);
        assert_eq!(LoadStatus::Loaded as u8, 2);
        assert_eq!(LoadStatus::Failed as u8, 3);
    }
}
