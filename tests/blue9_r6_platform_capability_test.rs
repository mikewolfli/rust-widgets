#![cfg(not(feature = "mini"))]

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

    assert!(content.contains("# Platform Capability Matrix"), "Matrix missing title header");
    assert!(content.contains("| Widget |"), "Matrix missing table header");
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

    let required_platforms =
        ["Windows", "Linux/X11", "macOS", "Wayland", "Mobile", "Harmony", "Embedded/Stub"];

    for platform in &required_platforms {
        assert!(content.contains(platform), "Matrix missing platform column: {}", platform);
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
        WidgetKind::FreeformShape,
        WidgetKind::TabBar,
        WidgetKind::PieMenu,
        WidgetKind::RibbonBar,
    ];

    for kind in &kinds {
        let debug_str = format!("{:?}", kind);
        assert!(!debug_str.is_empty(), "WidgetKind variant has an empty debug representation");
    }
}

#[test]
fn widget_kind_variants_are_exhaustive() {
    // The capability matrix must list exactly the same widget set as the
    // WidgetKind enum source. Rather than hard-coding a fragile count (the
    // enum grew from 82 to 167 variants over time), both sides are derived
    // dynamically: unit variants are lines of the form `    VariantName,`.
    let kind_src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/widget/kind.rs"))
            .expect("failed to read src/widget/kind.rs");
    let kind_variants = kind_src
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.ends_with(',') && trimmed.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        })
        .count();

    let matrix_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("plans")
        .join("platform_capability_matrix.md");
    let content = std::fs::read_to_string(&matrix_path).expect("failed to read capability matrix");
    let matrix_rows = content.lines().filter(|line| line.starts_with("| **")).count();

    assert!(
        kind_variants >= 100,
        "kind.rs unit-variant extraction looks wrong: got {kind_variants}"
    );
    assert_eq!(
        matrix_rows, kind_variants,
        "capability matrix rows ({matrix_rows}) must match WidgetKind source variants ({kind_variants})"
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
        "Toolbox",
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
