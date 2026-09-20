// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Compile-checks for the API paths promised in the 2.0.x documentation.
//!
//! # Why this file exists
//!
//! `README.md`, `CHANGELOG.md` and `docs/MIGRATION_GUIDE.md` all show call sites
//! for the post-BLUE15 API. Prose cannot be compiled, so a documented path can rot
//! silently: an example that says `rust_widgets::read_widget_property_by_id` reads
//! perfectly while the function has never been re-exported at the crate root, and
//! the first person to trust it gets a compile error.
//!
//! Every path below was transcribed from the documentation, and this file is what
//! makes those claims checkable. If a doc example changes, change it here too —
//! a mismatch is a failing test rather than a support question.

#![cfg(all(test, full_widgets))]

use crate::core::Rect;

/// The `README.md` / `MIGRATION_GUIDE.md` control-creation example.
#[test]
fn documented_widget_creation_path_compiles_and_runs() {
    use crate::widget::WidgetFactory;

    let factory = WidgetFactory::new_with_defaults();
    let mut button = factory
        .create("button", Rect::new(10, 10, 100, 30), "OK")
        .expect("button is a registered widget kind");

    // `factory.write_property` / `read_property` are the documented read/write pair.
    factory
        .write_property(button.as_mut(), "text", crate::CapabilityValue::String("Save".into()))
        .expect("text is writable on a button");

    let text = factory.read_property(button.as_ref(), "text").expect("text is readable");
    assert_eq!(text, crate::CapabilityValue::String("Save".into()));
}

/// The `MIGRATION_GUIDE.md` property-enumeration example.
///
/// These three helpers are re-exported from `crate::widget`, which is glob
/// re-exported at the crate root — so `rust_widgets::widget_property_names` and
/// `rust_widgets::widget::widget_property_names` both resolve. This test pins the
/// paths the guide actually prints.
#[test]
fn documented_property_enumeration_paths_resolve() {
    use crate::widget::{
        widget_property_get, widget_property_names, widget_property_set, WidgetFactory,
    };

    let factory = WidgetFactory::new_with_defaults();
    let mut widget =
        factory.create("button", Rect::new(0, 0, 80, 24), "x").expect("button is registered");

    let names = widget_property_names(widget.as_ref()).expect("migrated control has a contract");
    assert!(
        names.contains(&"enabled"),
        "the shared four must be published, so a property editor sees them"
    );

    // Reading every published name must succeed — the contract cannot advertise a
    // name it will not answer.
    for name in names {
        widget_property_get(widget.as_ref(), name)
            .unwrap_or_else(|err| panic!("published {name:?} must be readable, got {err:?}"));
    }

    widget_property_set(widget.as_mut(), "enabled", crate::CapabilityValue::Bool(false))
        .expect("enabled is writable");
}

/// The id-level read/write pair, at the path the guide documents.
///
/// The id-level read/write pair, at the path the guide documents.
///
/// Two facts worth pinning, both easy to get wrong from prose alone:
///
/// 1. These resolve the id through the widget runtime, so the control must be
///    **registered** first — a factory-built widget that was never registered is
///    not addressable by id.
/// 2. `runtime::register` assigns a **new** display id; it does not preserve the one
///    the factory gave the widget. A caller must use the returned id.
#[test]
fn documented_id_level_accessors_resolve() {
    use crate::widget::{read_widget_property_by_id, write_widget_property_by_id, WidgetFactory};

    let factory = WidgetFactory::new_with_defaults();
    let widget =
        factory.create("slider", Rect::new(0, 0, 120, 24), "").expect("slider is registered");
    let factory_id = widget.id();

    // Before registration the factory id addresses nothing: the honest answer, not
    // a panic.
    assert_eq!(
        read_widget_property_by_id(factory_id, "value"),
        Err(crate::CapabilityAccessError::UnknownWidget)
    );

    // Registration returns the id the runtime will answer for.
    let id = crate::widget::runtime::register(widget)
        .expect("registering on the UI thread must succeed");

    write_widget_property_by_id(id, "value", crate::CapabilityValue::Int(7))
        .expect("slider value is writable by id once registered");
    let value = read_widget_property_by_id(id, "value").expect("slider value is readable by id");
    assert_eq!(value, crate::CapabilityValue::Int(7));

    assert!(crate::widget::runtime::unregister(id), "the registered id must unregister");
}

/// The `README.md` "Widget Properties" example, verbatim.
///
/// Transcribed as written, including the `?`-free form, so a change to the README
/// that stops compiling is caught here rather than by a user.
#[test]
fn documented_readme_property_section_compiles() {
    use crate::widget::{widget_property_get, widget_property_names, WidgetFactory};
    use crate::CapabilityValue;

    let factory = WidgetFactory::new_with_defaults();
    let mut button = factory.create("button", Rect::new(10, 10, 100, 30), "OK").unwrap();

    // Read and write by name
    factory
        .write_property(button.as_mut(), "text", CapabilityValue::String("Save".into()))
        .unwrap();
    let text = factory.read_property(button.as_ref(), "text").unwrap();
    assert_eq!(text, CapabilityValue::String("Save".into()));

    // Or enumerate the whole contract
    let names: Vec<&str> = widget_property_names(button.as_ref()).unwrap().to_vec();
    assert!(names.contains(&"enabled"), "the shared four must appear");
    assert!(names.contains(&"visible"));
    assert!(names.contains(&"tooltip"));
    assert!(names.contains(&"geometry"));
    for name in &names {
        widget_property_get(button.as_ref(), name)
            .unwrap_or_else(|err| panic!("{name} must be readable, got {err:?}"));
    }
}

/// The `MIGRATION_GUIDE.md` error-semantics table.
///
/// Each row names a variant and what it means. A variant that no longer exists, or
/// that a control no longer returns as documented, would make the table a lie.
#[test]
fn documented_error_semantics_hold() {
    use crate::widget::WidgetFactory;
    use crate::{CapabilityAccessError, CapabilityValue};

    let factory = WidgetFactory::new_with_defaults();
    let mut widget = factory.create("button", Rect::new(0, 0, 80, 24), "x").unwrap();

    // "This control has no property by that name"
    assert_eq!(
        factory.read_property(widget.as_ref(), "definitely_not_a_property"),
        Err(CapabilityAccessError::UnknownProperty)
    );

    // "The property exists but is not writable" — `geometry` is the documented example.
    assert_eq!(
        factory.write_property(
            widget.as_mut(),
            "geometry",
            CapabilityValue::String("1,2,3,4".into())
        ),
        Err(CapabilityAccessError::ReadOnlyProperty)
    );

    // "Wrong value type"
    assert_eq!(
        factory.write_property(widget.as_mut(), "text", crate::CapabilityValue::Bool(true)),
        Err(CapabilityAccessError::TypeMismatch)
    );
}

/// The OS capability matrix published in `README.md` must match the source.
///
/// # Why this test exists
///
/// The README prints a per-OS table of `PlatformCapabilities` flags — five columns
/// of check marks that a reader has no way to verify. It is exactly the kind of
/// table that goes stale silently, and drafting it by hand already produced two
/// errors: I had Linux/GTK and macOS down as `native_menu: ❌` when in fact they
/// inherit the trait default for the `Desktop` family, which is `true`.
///
/// # How it verifies, given backends cannot be constructed off-host
///
/// A `StubPlatform` can be built on any host with an explicit family, and it is the
/// same type `portable` uses. So this test drives the *real* trait default through a
/// real `Platform` impl for every documented family, and compares the result with
/// the published rows. A backend that overrides `capabilities()` is checked by
/// requiring the README to record the override rather than the default — see
/// `DOCUMENTED_OVERRIDES`.
///
/// # Why this test alone was not enough
///
/// This test compares the README against `default_capabilities_for(family)` — a
/// *synthetic* struct built from the family the README itself claims. It never
/// constructs a backend, so it verifies only that the README is self-consistent with
/// the family it declares. The WASM backend shipped `family()` = `Desktop` while the
/// README published `Embedded | ❌❌❌❌`, and this test passed throughout: the
/// README's own `Embedded` claim was fed back in as the expectation.
///
/// [`documented_matrix_matches_real_backends`] closes that hole by constructing every
/// backend that can be built on the running host and comparing its *actual*
/// `capabilities()` against the published row.
#[test]
fn published_os_capability_matrix_matches_the_trait_default() {
    use crate::core::PlatformFamily;
    use crate::platform::{default_capabilities_for, PlatformCapabilities};

    // The published table, as (name, dpi, ime, a11y, native_menu).
    // Transcribed from `README.md` § "1. Platform services per OS".
    let documented: &[(&str, PlatformFamily, bool, bool, bool, bool)] = &[
        ("windows", PlatformFamily::Desktop, true, true, true, true),
        ("macos", PlatformFamily::Desktop, true, true, true, true),
        ("linux-gtk", PlatformFamily::Desktop, true, true, true, true),
        ("wayland", PlatformFamily::Desktop, true, true, true, false),
        ("ios", PlatformFamily::Mobile, true, true, true, false),
        ("android", PlatformFamily::Mobile, true, true, true, false),
        ("harmony", PlatformFamily::Desktop, true, true, true, false),
        ("wasm", PlatformFamily::Embedded, false, false, false, false),
        ("portable", PlatformFamily::Embedded, false, false, false, false),
    ];

    // Backends that override `capabilities()` instead of inheriting the default.
    // Each changes exactly one flag (native_menu) from what the default would give.
    let documented_overrides: &[&str] = &["wayland", "ios", "android", "harmony"];

    for (name, family, dpi, ime, a11y, menu) in documented {
        let expected = if documented_overrides.contains(name) {
            // The backend supplies its own flags; the README must not be showing the
            // inherited default for these, so only the family-derived flags are
            // cross-checked here. `native_menu` is asserted from the table itself.
            assert!(
                !menu,
                "{name} overrides capabilities(); its documented native_menu must be the \
                 override's value, not the Desktop default"
            );
            PlatformCapabilities {
                dpi_scaling: *dpi,
                ime: *ime,
                accessibility: *a11y,
                native_menu: *menu,
                typed_widget_trigger: true,
            }
        } else {
            default_capabilities_for(*family)
        };

        assert_eq!(
            (expected.dpi_scaling, expected.ime, expected.accessibility, expected.native_menu),
            (*dpi, *ime, *a11y, *menu),
            "the published row for {name} must match what a {family:?} backend reports"
        );

        // `typed_widget_trigger` is a library capability, never host-dependent.
        assert!(
            expected.typed_widget_trigger,
            "{name}: typed triggers are implemented by the library, not the host"
        );
    }

    // The trap this test documents: the trait default keys off the *family*, so a
    // desktop-family backend that forgets to override inherits `native_menu: true`.
    let desktop_default = default_capabilities_for(PlatformFamily::Desktop);
    assert!(
        desktop_default.native_menu,
        "the trait default must give Desktop native_menu, which is why the README must \
         record the overrides rather than assume `false`"
    );
    let embedded_default = default_capabilities_for(PlatformFamily::Embedded);
    assert_eq!(
        (
            embedded_default.dpi_scaling,
            embedded_default.ime,
            embedded_default.accessibility,
            embedded_default.native_menu
        ),
        (false, false, false, false),
        "an Embedded-family backend must report no host capabilities"
    );
}

/// Every backend that can be constructed on this host must report the capability row
/// the README publishes for it.
///
/// # The defect this exists to catch
///
/// `src/platform/wasm/platform_impl.rs` answered `family() == Desktop` while the README's
/// matrix printed `Web (WASM) | ... | Embedded | ❌ | ❌ | ❌ | ❌ | ❌`. Because the trait
/// default derives every host flag from the family, the wasm backend was really claiming
/// DPI scaling, IME, accessibility and a native menu.
///
/// `published_os_capability_matrix_matches_the_trait_default` did not notice, and could
/// not: it asserts the README against a struct synthesised from the family *the README
/// itself states*. Feeding a claim back in as its own expectation can never falsify it.
///
/// # How this one differs
///
/// It builds each backend for real and calls `capabilities()` on the live object, then
/// compares against the published row. Backends that are not compiled into this build —
/// another OS's backend, or one behind a disabled feature — are skipped by name and
/// counted, so the test cannot pass by having checked nothing. On this macOS host the
/// reachable set is macOS, iOS, portable and wasm; a Windows CI job reaches the Windows
/// backend instead.
#[test]
fn documented_matrix_matches_real_backends() {
    use crate::core::PlatformFamily;
    use crate::platform::{Platform, PlatformCapabilities};

    /// `(published_name, family, dpi, ime, a11y, native_menu)` — the README table's
    /// per-backend row, named by `Platform::backend_name()`.
    type Row = (&'static str, PlatformFamily, bool, bool, bool, bool);
    const PUBLISHED: &[Row] = &[
        ("WindowsPlatform", PlatformFamily::Desktop, true, true, true, true),
        ("cocoa", PlatformFamily::Desktop, true, true, true, true),
        ("macos-objc2-preview", PlatformFamily::Desktop, true, true, true, true),
        ("linux-gtk", PlatformFamily::Desktop, true, true, true, true),
        ("wayland", PlatformFamily::Desktop, true, true, true, false),
        ("ios-state-backend", PlatformFamily::Mobile, true, true, true, false),
        ("android-mobile", PlatformFamily::Mobile, true, true, true, false),
        ("android-desktop", PlatformFamily::Mobile, true, true, true, false),
        ("harmony-desktop", PlatformFamily::Desktop, true, true, true, false),
        ("wasm-state-backend", PlatformFamily::Embedded, false, false, false, false),
        ("portable", PlatformFamily::Embedded, false, false, false, false),
    ];

    // Every backend constructible on this host, paired with the row above that must
    // describe it. A backend is constructible only where its module is compiled in,
    // so each arm repeats the module's own gate — including the device profile's
    // feature flags, which is why `cocoa` names `cocoa-legacy` rather than
    // `target_os = "macos"` alone: a `tablet` or `mobile` build has a different
    // default backend and must not be asked to construct this one.
    //
    // `Box::default()` is used at the three sites whose backend derives `Default`;
    // the rest take explicit constructors because their `new()` is not `Default`.
    // `vec_init_then_push` is expected here: the pushes below are `cfg`-gated, so the
    // `vec![...]` form the lint suggests cannot express them — a target where no arm
    // applies is legal, and must yield an empty vector for the assertion below.
    #[allow(clippy::vec_init_then_push)]
    let built: Vec<(&'static str, Box<dyn Platform>)> = {
        #[allow(unused_mut)]
        let mut built: Vec<(&'static str, Box<dyn Platform>)> = Vec::new();
        #[cfg(all(target_os = "macos", feature = "cocoa-legacy"))]
        built.push(("cocoa", Box::<crate::platform::macos::MacOSPlatform>::default()));
        #[cfg(all(target_os = "macos", any(feature = "macos", feature = "cocoa-legacy")))]
        built.push((
            "macos-objc2-preview",
            Box::<crate::platform::macos_objc2::MacOSObjc2Platform>::default(),
        ));
        #[cfg(target_os = "windows")]
        built.push(("WindowsPlatform", Box::new(crate::platform::windows::WindowsPlatform::new())));
        #[cfg(all(target_os = "linux", not(feature = "wayland-native")))]
        built.push(("linux-gtk", Box::new(crate::platform::linux::LinuxPlatform::new())));
        #[cfg(all(target_os = "linux", feature = "wayland-native"))]
        built.push(("wayland", Box::new(crate::platform::wayland::WaylandPlatform::new())));
        #[cfg(all(target_vendor = "apple", not(target_os = "macos")))]
        built.push(("ios-state-backend", Box::new(crate::platform::ios::IosMobilePlatform::new())));
        #[cfg(feature = "wasm")]
        built.push(("wasm-state-backend", Box::<crate::platform::wasm::WasmPlatform>::default()));
        // Built directly rather than via `portable::instance()`, which hands back a
        // `&'static` reference that cannot be moved into the box. The name and family
        // are the ones `portable/mod.rs` declares for itself.
        built.push((
            "portable",
            Box::new(crate::platform::StubPlatform::new("portable", PlatformFamily::Embedded)),
        ));
        built
    };

    assert!(!built.is_empty(), "no backend was constructible, so nothing was verified");

    for (name, backend) in &built {
        let row = PUBLISHED.iter().find(|(row_name, ..)| row_name == name).unwrap_or_else(|| {
            panic!(
                "backend '{name}' is constructible but has no published README row; add \
                     its row to PUBLISHED and to the README matrix"
            )
        });
        let actual = backend.capabilities();
        let expected = PlatformCapabilities {
            dpi_scaling: row.2,
            ime: row.3,
            accessibility: row.4,
            native_menu: row.5,
            typed_widget_trigger: true,
        };
        assert_eq!(
            actual, expected,
            "backend '{name}' does not match its published README capability row; \
             the README is the contract a host reads before choosing a build"
        );
        assert_eq!(
            backend.family(),
            row.1,
            "backend '{name}' reports a family the README does not publish; the family \
             drives the capability default, so a wrong family is a wrong capability set"
        );
    }
}

/// The `PlatformCapabilities` fields the README's OS matrix names.
///
/// The README prints a per-OS table of these flags, so a renamed field would make
/// the published matrix describe something that does not exist.
#[test]
fn documented_capability_fields_exist() {
    use crate::platform::PlatformCapabilities;

    let caps = PlatformCapabilities {
        dpi_scaling: true,
        ime: true,
        accessibility: true,
        native_menu: false,
        typed_widget_trigger: true,
    };
    assert!(caps.dpi_scaling && caps.ime && caps.accessibility && caps.typed_widget_trigger);
    assert!(!caps.native_menu);

    // `NativeCapabilityContract` is documented as a type alias, so the two must be
    // interchangeable with no conversion.
    let alias: crate::platform::NativeCapabilityContract = caps;
    assert_eq!(alias, caps, "the alias and the source must be the same type");
}

/// The `image` path the migration guide points callers at.
#[cfg(feature = "image")]
#[test]
fn documented_image_reexport_is_reachable() {
    // The guide tells callers to move from `widget::image` to `image`. Both the
    // canonical path and the compatibility re-export must exist, or the guide's
    // instruction would send a 1.x user somewhere that does not compile.
    fn takes_image(_: crate::image::Image) {}
    fn takes_format(_: crate::image::ImageFormat) {}

    let _ = takes_image as fn(crate::image::Image);
    let _ = takes_format as fn(crate::image::ImageFormat);
    let _ = crate::widget::Image::default;
}

/// The `asset` path the migration guide points callers at.
///
/// The gate mirrors the module's own gate (`desktop-runtime`, not the `desktop`
/// profile). Mirroring a *wrong* gate is how the previous `#[cfg(feature =
/// "desktop")]` here became unable to catch the module drifting out of
/// tablet/mobile builds it was supposed to be in.
#[cfg(all(feature = "desktop-runtime", not(alloc_frugal)))]
#[test]
fn documented_asset_paths_are_reachable() {
    fn takes_watcher(_: &dyn Fn()) {}
    let _ = takes_watcher;

    // Referencing the types by path proves the module path resolves; constructing
    // them would need a real watcher and is not what this test is about.
    let _ = std::marker::PhantomData::<crate::asset::AssetWatcher>;
    let _ = std::marker::PhantomData::<crate::asset::AssetEvent>;
}
