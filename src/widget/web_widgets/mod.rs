// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Web-related widgets: web engines, web views, etc.
pub mod web_engine;
pub mod web_view;
pub use web_engine::WebEngineView as WebEngine;
pub use web_engine::{
    WebEngineContextMenuRequest, WebEngineCookieStore, WebEngineDownloadItem,
    WebEngineFindTextResult, WebEngineNotification, WebEnginePage, WebEngineScriptDialog,
    WebEngineSettings, WebEngineView, WebEngineWebChannel,
};
pub use web_view::WebView;
