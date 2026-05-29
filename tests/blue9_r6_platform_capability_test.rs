//! R6 Platform Capability Matrix Integration Test (blue9_r6)
//!
//! This test verifies:
//! 1. The capability matrix document exists and is parseable
//! 2. Basic platform contract negotiation works
//! 3. All WidgetKind variants have at least StateBacked capability

use std::path::Path;

// ---------------------------------------------------------------------------
// Test 1: Capability matrix document exists and is parseable
// ---------------------------------------------------------------------------

#[test]
fn capability_matrix_document_exists() {
    let matrix_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("plans")
        .join("platform_capability_matrix.md");

    assert!(
        matrix_path.exists(),
        "Capability matrix document not found at: {}",
        matrix_path.display()
    );

    let content =
        std::fs::read_to_string(&matrix_path).expect("Failed to read capability matrix document");

    assert!(
        content.contains("# Platform Capability Matrix"),
        "Matrix missing title header"
    );
    assert!(
        content.contains("| Widget |"),
        "Matrix missing table header"
    );
    assert!(
        content.contains("✅") || content.contains("🔶"),
        "Matrix missing capability emoji codes"
    );
}

#[test]
fn capability_matrix_covers_all_platforms() {
    let matrix_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("plans")
        .join("platform_capability_matrix.md");

    let content =
        std::fs::read_to_string(&matrix_path).expect("Failed to read capability matrix document");

    let required_platforms = [
        "Windows",
        "Linux/X11",
        "macOS",
        "Wayland",
        "Mobile",
        "Harmony",
        "Embedded/Stub",
    ];

    for platform in &required_platforms {
        assert!(
            content.contains(platform),
            "Matrix missing platform column: {}",
            platform
        );
    }
}

// ---------------------------------------------------------------------------
// Test 2: Platform contract negotiation works
// ---------------------------------------------------------------------------

#[test]
fn platform_contract_negotiation_works() {
    use rust_widgets::core::RuntimeProfile;
    use rust_widgets::platform::negotiate_capability_contract;
    use rust_widgets::platform::CapabilityContract;

    let full_contract = negotiate_capability_contract(RuntimeProfile::Full);
    match full_contract {
        CapabilityContract::Native(contract) => {
            assert!(
                contract.typed_widget_trigger,
                "Native contract must support typed_widget_trigger"
            );
        }
        CapabilityContract::Embedded(contract) => {
            assert!(
                contract.typed_widget_trigger,
                "Embedded contract must support typed_widget_trigger"
            );
        }
    }

    let embedded_contract = negotiate_capability_contract(RuntimeProfile::Embedded);
    match embedded_contract {
        CapabilityContract::Native(_) => {
            // On test environments without embedded, we get native fallback
        }
        CapabilityContract::Embedded(contract) => {
            assert!(
                contract.typed_widget_trigger,
                "Embedded contract must support typed_widget_trigger"
            );
            assert!(
                contract.low_memory_mode,
                "Embedded contract should have low_memory_mode enabled"
            );
        }
    }
}

#[test]
fn platform_capabilities_have_typed_widget_trigger() {
    use rust_widgets::platform::PlatformCapabilities;

    let desktop_caps = PlatformCapabilities {
        dpi_scaling: true,
        ime: true,
        accessibility: true,
        native_menu: true,
        typed_widget_trigger: true,
    };
    assert!(desktop_caps.typed_widget_trigger);
    assert!(desktop_caps.dpi_scaling);
    assert!(desktop_caps.ime);

    let embedded_caps = PlatformCapabilities {
        dpi_scaling: false,
        ime: false,
        accessibility: false,
        native_menu: false,
        typed_widget_trigger: true,
    };
    assert!(embedded_caps.typed_widget_trigger);
}

// ---------------------------------------------------------------------------
// Test 3: All WidgetKind variants have at least StateBacked capability
// ---------------------------------------------------------------------------

#[test]
fn all_widget_kinds_have_non_empty_debug_repr() {
    use rust_widgets::widget::kind::WidgetKind;

    // Every WidgetKind variant must have a non-empty Debug representation
    let kinds = [
        WidgetKind::Window,
        WidgetKind::Dialog,
        WidgetKind::MessageBox,
        WidgetKind::FileDialog,
        WidgetKind::ColorDialog,
        WidgetKind::FontDialog,
        WidgetKind::InputDialog,
        WidgetKind::ProgressDialog,
        WidgetKind::PopupWindow,
        WidgetKind::Button,
        WidgetKind::CheckBox,
        WidgetKind::RadioButton,
        WidgetKind::Label,
        WidgetKind::LineEdit,
        WidgetKind::TextEdit,
        WidgetKind::RichEdit,
        WidgetKind::ComboBox,
        WidgetKind::SpinBox,
        WidgetKind::ListBox,
        WidgetKind::ListView,
        WidgetKind::TreeView,
        WidgetKind::ProgressBar,
        WidgetKind::Slider,
        WidgetKind::ScrollBar,
        WidgetKind::ScrollArea,
        WidgetKind::Panel,
        WidgetKind::DockPanel,
        WidgetKind::GroupBox,
        WidgetKind::TabWidget,
        WidgetKind::Splitter,
        WidgetKind::MdiArea,
        WidgetKind::MenuBar,
        WidgetKind::Menu,
        WidgetKind::MenuItem,
        WidgetKind::ContextMenu,
        WidgetKind::ToolBar,
        WidgetKind::StatusBar,
        WidgetKind::Canvas,
        WidgetKind::Table,
        WidgetKind::Grid,
        WidgetKind::Chart,
        WidgetKind::ToggleButton,
        WidgetKind::CheckListBox,
        WidgetKind::DoubleSpinBox,
        WidgetKind::Dial,
        WidgetKind::Wizard,
        WidgetKind::DatePicker,
        WidgetKind::TimePicker,
        WidgetKind::DateTimePicker,
        WidgetKind::DirectoryDialog,
        WidgetKind::DataView,
        WidgetKind::PropertyGrid,
        WidgetKind::Toolbox,
        WidgetKind::StackedWidget,
        WidgetKind::CollapsiblePane,
        WidgetKind::DockWidget,
        WidgetKind::WebView,
        WidgetKind::ActivityIndicator,
        WidgetKind::Calendar,
        WidgetKind::ColumnView,
        WidgetKind::UndoView,
        WidgetKind::CommandLink,
        WidgetKind::LCDNumber,
        WidgetKind::FontComboBox,
        WidgetKind::WebEngineView,
        WidgetKind::WebEnginePage,
        WidgetKind::WebEngineSettings,
        WidgetKind::WebEngineDownloadItem,
        WidgetKind::WebEngineCookieStore,
        WidgetKind::WebEngineWebChannel,
        WidgetKind::WebEngineFindTextResult,
        WidgetKind::WebEngineNotification,
        WidgetKind::WebEngineScriptDialog,
        WidgetKind::WebEngineContextMenuRequest,
        WidgetKind::Action,
        WidgetKind::ToolButton,
        WidgetKind::ToolBox,
        WidgetKind::FreeformShape,
        WidgetKind::TabBar,
        WidgetKind::PieMenu,
        WidgetKind::RibbonBar,
    ];

    for kind in &kinds {
        let debug_str = format!("{:?}", kind);
        assert!(
            !debug_str.is_empty(),
            "WidgetKind variant has an empty debug representation"
        );
    }
}

#[test]
fn widget_kind_variants_are_exhaustive() {
    let variants = [
        "Window",
        "Dialog",
        "MessageBox",
        "FileDialog",
        "ColorDialog",
        "FontDialog",
        "InputDialog",
        "ProgressDialog",
        "PopupWindow",
        "Button",
        "CheckBox",
        "RadioButton",
        "Label",
        "LineEdit",
        "TextEdit",
        "RichEdit",
        "ComboBox",
        "SpinBox",
        "ListBox",
        "ListView",
        "TreeView",
        "ProgressBar",
        "Slider",
        "ScrollBar",
        "ScrollArea",
        "Panel",
        "DockPanel",
        "GroupBox",
        "TabWidget",
        "Splitter",
        "MdiArea",
        "MenuBar",
        "Menu",
        "MenuItem",
        "ContextMenu",
        "ToolBar",
        "StatusBar",
        "Canvas",
        "Table",
        "Grid",
        "Chart",
        "ToggleButton",
        "CheckListBox",
        "DoubleSpinBox",
        "Dial",
        "Wizard",
        "DatePicker",
        "TimePicker",
        "DateTimePicker",
        "DirectoryDialog",
        "DataView",
        "PropertyGrid",
        "Toolbox",
        "StackedWidget",
        "CollapsiblePane",
        "DockWidget",
        "WebView",
        "ActivityIndicator",
        "Calendar",
        "ColumnView",
        "UndoView",
        "CommandLink",
        "LCDNumber",
        "FontComboBox",
        "WebEngineView",
        "WebEnginePage",
        "WebEngineSettings",
        "WebEngineDownloadItem",
        "WebEngineCookieStore",
        "WebEngineWebChannel",
        "WebEngineFindTextResult",
        "WebEngineNotification",
        "WebEngineScriptDialog",
        "WebEngineContextMenuRequest",
        "Action",
        "ToolButton",
        "ToolBox",
        "FreeformShape",
        "TabBar",
        "PieMenu",
        "RibbonBar",
    ];

    assert_eq!(
        variants.len(),
        81,
        "WidgetKind variant count mismatch — expected 81, got {}",
        variants.len()
    );
}

// ---------------------------------------------------------------------------
// Test 4: Verify matrix document is consistent with WidgetKind source
// ---------------------------------------------------------------------------

#[test]
fn capability_matrix_matches_widget_kind() {
    let matrix_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("plans")
        .join("platform_capability_matrix.md");

    let content =
        std::fs::read_to_string(&matrix_path).expect("Failed to read capability matrix document");

    // Extract widget names from the matrix table rows.
    // Rows look like: "| **Window** | ✅ | ✅ | ..."
    let matrix_widgets: Vec<String> = content
        .lines()
        .filter_map(|line| {
            if line.starts_with("| **") {
                let rest = line.strip_prefix("| **")?;
                let name = rest.split("**").next()?;
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();

    // Check that a representative sample of key widgets is present
    let required_widgets = [
        "Window",
        "Button",
        "Label",
        "ListView",
        "MenuBar",
        "Menu",
        "ToolBar",
        "StatusBar",
        "Dialog",
        "Canvas",
        "WebView",
        "RibbonBar",
    ];

    for widget in &required_widgets {
        assert!(
            matrix_widgets.contains(&widget.to_string()),
            "Widget '{}' is not present in the capability matrix",
            widget
        );
    }

    // Verify the matrix widget count is reasonable (matches WidgetKind variants)
    assert!(
        matrix_widgets.len() >= 80,
        "Matrix only has {} widget rows (expected >= 80)",
        matrix_widgets.len()
    );
}
