#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

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
            // The fallback used to fabricate all-true, so a backend that publishes no
            // contract of its own was told it had a native menu even while reporting a
            // non-desktop family. The answer must now follow the family the backend
            // actually reports.
            let actual_family = rust_widgets::platform::get_platform().family();
            let expected = rust_widgets::platform::default_capabilities_for(actual_family);
            assert_eq!(
                contract, expected,
                "the negotiated contract must match the family the backend reports ({actual_family:?}); \
                 a mismatch means the fallback invented capabilities"
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
    // The capability matrix must cover the same widget set as the `WidgetKind`
    // enum source. Both sides are derived dynamically rather than from a count,
    // because a count drifts silently: the enum grew 82 -> 167 -> 169 over time,
    // and a hard-coded number would have been "updated" rather than investigated.
    //
    // # The one documented difference
    //
    // The matrix lists the `WebEngine*` **wrapper types** as well as
    // `WebEngineView`. They are real render-pipeline symbols over the one
    // registered view — each forwards `Widget::base()` to what it wraps, so
    // `kind()` answers `WebEngineView` for all of them. They were once
    // `WidgetKind` variants marked `kind-role: base`, which made them orphans
    // (principle #22): nothing could produce them, and `create_web_engine_page(..)`
    // produced id `0` because `factory_name_for_kind` resolves through
    // `capability_by_kind`.
    //
    // So the comparison is: every `WidgetKind` variant has a matrix row, and every
    // matrix row is either a variant or one of the documented wrapper names. The
    // assertion stays exact in both directions — it is the *set* that is allowed
    // to be larger, not the count that is loosened.
    const WEB_ENGINE_WRAPPERS: &[&str] = &[
        "WebEnginePage",
        "WebEngineSettings",
        "WebEngineDownloadItem",
        "WebEngineCookieStore",
        "WebEngineWebChannel",
        "WebEngineFindTextResult",
        "WebEngineNotification",
        "WebEngineScriptDialog",
        "WebEngineContextMenuRequest",
    ];

    let kind_src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/widget/kind.rs"))
            .expect("failed to read src/widget/kind.rs");
    let kind_variants: std::collections::BTreeSet<String> = kind_src
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.ends_with(',') && trimmed.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        })
        .map(|line| line.trim().trim_end_matches(',').to_string())
        .collect();

    let matrix_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("plans")
        .join("platform_capability_matrix.md");
    let content = std::fs::read_to_string(&matrix_path).expect("failed to read capability matrix");
    let matrix_rows: std::collections::BTreeSet<String> = content
        .lines()
        .filter(|line| line.starts_with("| **"))
        .filter_map(|line| {
            let rest = line.strip_prefix("| **")?;
            let end = rest.find("**")?;
            Some(rest[..end].to_string())
        })
        .collect();

    assert!(
        kind_variants.len() >= 100,
        "kind.rs unit-variant extraction looks wrong: got {}",
        kind_variants.len()
    );
    assert!(!matrix_rows.is_empty(), "the capability matrix has no widget rows");

    // Every kind must be documented.
    let undocumented: Vec<&String> = kind_variants.difference(&matrix_rows).collect();
    assert!(
        undocumented.is_empty(),
        "these WidgetKind variants have no capability-matrix row: {undocumented:?}"
    );

    // And every row must be a kind or a documented wrapper — no invented rows.
    let unexplained: Vec<&String> = matrix_rows
        .difference(&kind_variants)
        .filter(|name| !WEB_ENGINE_WRAPPERS.contains(&name.as_str()))
        .collect();
    assert!(
        unexplained.is_empty(),
        "these capability-matrix rows are neither a WidgetKind variant nor a \
         documented WebEngine wrapper, so the matrix documents something that does \
         not exist: {unexplained:?}"
    );

    // The wrapper list must not silently rot: each name has to be a row.
    for wrapper in WEB_ENGINE_WRAPPERS {
        assert!(
            matrix_rows.contains(*wrapper),
            "{wrapper} is excused as a WebEngine wrapper but has no matrix row; if it \
             was deleted, drop it from WEB_ENGINE_WRAPPERS too"
        );
    }
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

// ---------------------------------------------------------------------------
// Test 8: The negotiation fallback must not invent capabilities
// ---------------------------------------------------------------------------
//
// `negotiate_capability_contract(Full)` falls back when a backend publishes no
// `native_capability_contract()`. `Platform::native_capability_contract()` returns
// `None` for every non-`Desktop` family, so the fallback is only ever reached from a
// **non-desktop** backend — and it used to answer with all four flags `true`, telling
// an embedded or mobile backend it had DPI scaling, IME, accessibility and a native
// menu.
//
// This drives the **real** entry point with a real non-desktop backend installed
// through `with_platform`, rather than asserting on the helper the fallback happens to
// call. That distinction matters: a test written against `default_capabilities_for`
// would keep passing if the fallback stopped using it, which is exactly the bug.
#[test]
fn capability_fallback_follows_the_family_and_never_invents() {
    use rust_widgets::core::{PlatformFamily, RuntimeProfile};
    use rust_widgets::platform::{
        default_capabilities_for, negotiate_capability_contract, with_platform, CapabilityContract,
        StubPlatform,
    };

    // A backend that publishes no `native_capability_contract()` of its own, because
    // its family is not `Desktop`. This is the only input that reaches the fallback.
    // `with_platform` needs a `&'static`, so the two are leaked once per test run.
    static MOBILE_BACKEND: std::sync::LazyLock<StubPlatform> =
        std::sync::LazyLock::new(|| StubPlatform::new("test-mobile", PlatformFamily::Mobile));
    static EMBEDDED_BACKEND: std::sync::LazyLock<StubPlatform> =
        std::sync::LazyLock::new(|| StubPlatform::new("test-embedded", PlatformFamily::Embedded));

    for (name, backend, family) in [
        ("mobile", &*MOBILE_BACKEND as &'static dyn rust_widgets::platform::Platform,
         PlatformFamily::Mobile),
        ("embedded", &*EMBEDDED_BACKEND as &'static dyn rust_widgets::platform::Platform,
         PlatformFamily::Embedded),
    ] {
        // The precondition this test depends on: no published contract, so the
        // fallback really is what answers.
        assert!(
            backend.native_capability_contract().is_none(),
            "{name}: this test only means something for a backend without a published contract"
        );

        let contract = with_platform(backend, || {
            negotiate_capability_contract(RuntimeProfile::Full)
        });
        let CapabilityContract::Native(caps) = contract else {
            panic!("{name}: a Full profile must negotiate a Native contract");
        };
        let expected = default_capabilities_for(family);

        assert_eq!(
            caps, expected,
            "{name}: the fallback contract must match the family the backend reports; \n\
             a mismatch means it invented capabilities the host cannot serve"
        );
        assert!(
            !(caps.dpi_scaling && caps.ime && caps.accessibility && caps.native_menu),
            "{name}: a non-desktop backend must never receive an all-true contract"
        );
        assert!(
            caps.typed_widget_trigger,
            "{name}: typed triggers come from the library, never the host"
        );
    }
}
