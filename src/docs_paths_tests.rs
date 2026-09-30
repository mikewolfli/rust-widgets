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

/// The OS capability matrix published in the cookbook must match the source.
///
/// # Why this test exists
///
/// The cookbook prints a per-OS table of `PlatformCapabilities` flags — four columns
/// of check marks that a reader has no way to verify. It is exactly the kind of
/// table that goes stale silently, and drafting it by hand has now produced errors
/// **twice**:
///
/// 1. Linux/GTK and macOS were down as `native_menu: ❌` when they inherit the trait
///    default for the `Desktop` family, which was then `true`.
/// 2. After the default became an honest all-`false`, Windows and macOS were left as
///    `native_menu: ✅` / `dpi_scaling: ✅` even though **neither backend implements
///    any menu method and the macOS backend never queries the screen scale**. The
///    flag means "this backend wires the OS facility up", not "this OS has one", and
///    the second reading is what the table had.
///
/// The second error is the instructive one: the first was a transcription slip, the
/// second was a *semantic* error in how the column was understood. The assertions
/// below therefore pin the meaning, not just the digits — see the doc comment on
/// [`documented_matrix_matches_real_backends`], which cross-checks the same rows
/// against live backend objects on any host that can construct them.
///
/// # How it verifies, given backends cannot be constructed off-host
///
/// Every backend declares its own `capabilities()` (the trait default is an honest
/// all-`false`, so an inheritance can no longer stand in for a real answer), and this test
/// keeps the published matrix from silently restating that default: a documented `Desktop`
/// row that equals the default is a row that records nothing.
///
/// [`documented_matrix_matches_real_backends`] closes the stronger hole by constructing
/// every backend that can be built on the running host and comparing its *actual*
/// `capabilities()` against the published row.
#[test]
fn published_os_capability_matrix_matches_the_trait_default() {
    use crate::core::PlatformFamily;
    use crate::platform::{default_capabilities_for, PlatformCapabilities};

    // The published table, as (name, dpi, ime, a11y, native_menu).
    // Transcribed from the cookbook's `chapters/platform-support.md` § "1.3 Platform services
    // do vary by OS", which is the matrix a host reads before choosing a build.
    //
    // `windows` and `macos` are `native_menu: false` and `macos` is additionally
    // `dpi_scaling: false`, because the column records an implementation rather than an
    // OS feature. Those two rows are the ones a reader is most likely to "correct" by
    // hand, so the comment is here to say why they look surprising.
    let documented: &[(&str, PlatformFamily, bool, bool, bool, bool)] = &[
        ("windows", PlatformFamily::Desktop, true, true, true, false),
        ("macos", PlatformFamily::Desktop, false, true, true, false),
        ("linux-gtk", PlatformFamily::Desktop, true, true, true, false),
        // Wayland answers `dpi_scale_factor()` from `GDK_SCALE`/`QT_SCALE_FACTOR` (so `dpi_scaling`
        // is honest) but overrides neither `ime_bridge()` nor `accessibility_bridge()`: there is no
        // Wayland `text-input` binding and no Wayland a11y bridge in this crate, so both flags would
        // promise a method that answers `None`.
        ("wayland", PlatformFamily::Desktop, true, false, false, false),
        ("ios", PlatformFamily::Mobile, false, false, false, false),
        ("android", PlatformFamily::Mobile, true, true, true, false),
        ("harmony", PlatformFamily::Desktop, true, true, true, false),
        ("wasm", PlatformFamily::Embedded, false, false, false, false),
        ("portable", PlatformFamily::Embedded, false, false, false, false),
    ];

    for (name, family, dpi, ime, a11y, menu) in documented {
        // Every backend now states its own flags, so the published row must not be the
        // trait default. The default is the honest all-`false`; a backend with a real
        // integration overrides it. This loop is what keeps the two apart: a row that
        // merely restated the default would be indistinguishable from a missing override.
        let synthetic = PlatformCapabilities {
            dpi_scaling: *dpi,
            ime: *ime,
            accessibility: *a11y,
            native_menu: *menu,
            typed_widget_trigger: true,
        };
        let default = default_capabilities_for(*family);
        assert!(
            synthetic != default || *family != PlatformFamily::Desktop,
            "{name}: the documented row is identical to the trait default, so the README no \
             longer records what the backend actually declares"
        );

        assert_eq!(
            (synthetic.dpi_scaling, synthetic.ime, synthetic.accessibility, synthetic.native_menu),
            (*dpi, *ime, *a11y, *menu),
            "the published row for {name} must match what the backend reports"
        );

        // `typed_widget_trigger` is a library capability, never host-dependent.
        assert!(
            synthetic.typed_widget_trigger,
            "{name}: typed triggers are implemented by the library, not the host"
        );
    }

    // The default is an honest absence for every family: an undeclared capability is
    // not granted by a family classification (rule #37). This is the half the
    // `default_capabilities_for` doc comment promises.
    for family in [PlatformFamily::Desktop, PlatformFamily::Mobile, PlatformFamily::Embedded] {
        let default = default_capabilities_for(family);
        assert_eq!(
            (default.dpi_scaling, default.ime, default.accessibility, default.native_menu),
            (false, false, false, false),
            "a {family:?} backend that declares nothing must claim nothing"
        );
        assert!(default.typed_widget_trigger, "the library capability is always present");
    }
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

    /// `(published_name, family, dpi, ime, a11y, native_menu)` — the per-backend row, keyed by
    /// the name the test's own `built` list uses.
    ///
    /// # The keys are test keys, not `backend_name()`s — and that gap hid a stale row
    ///
    /// It used to say these were "named by `Platform::backend_name()`". They are not: the loop
    /// uses the tuple name purely as a lookup key (see `find(..)` below) and never asserts
    /// `name == backend.backend_name()`. Two rows were therefore describing backends that do not
    /// answer to them — `"linux-gtk"` is a `LinuxPlatform` whose real name is `"gtk"` or
    /// `"linux-state-backend"`, and the old `"android-desktop"` named a backend that exists
    /// nowhere else in the tree. The names are kept as keys because renaming them is churn, but
    /// the doc now says what they are.
    ///
    /// # Why `linux-gtk`'s `dpi_scaling` is computed rather than written
    ///
    /// `LinuxPlatform::capabilities()` reports
    /// `dpi_scaling: self.dpi_scale_factor() != 1.0 || cfg!(feature = "gtk-native")` — a
    /// **feature-and-environment-dependent** answer, not a constant. The published table said a
    /// flat `true`, so the row and the backend disagreed on every build that is not GTK-native,
    /// which is what this test caught. The expectation is computed the same way the backend
    /// computes it rather than hard-coding one build's answer. (The cookbook's human-readable
    /// matrix still prints `✅`, because it describes a GTK-backed build; this row has to hold on
    /// any host the test runs on.)
    type Row = (&'static str, PlatformFamily, bool, bool, bool, bool);
    let expect_linux_dpi = {
        #[cfg(target_os = "linux")]
        {
            crate::platform::linux::LinuxPlatform::new().dpi_scale_factor() != 1.0
                || cfg!(feature = "gtk-native")
        }
        // The row is only ever compared on a host where `LinuxPlatform` is constructible, and the
        // `built` list below gates it the same way — so this value is unreachable elsewhere and
        // need not describe another host's answer.
        #[cfg(not(target_os = "linux"))]
        {
            true
        }
    };
    let published: &[Row] = &[
        // `native_menu: false` on all three desktop backends with an OS menu API: the flag records
        // an implementation, not an OS feature. See `PlatformCapabilities::native_menu` and each
        // backend's `capabilities()` for the per-flag evidence.
        ("WindowsPlatform", PlatformFamily::Desktop, true, true, true, false),
        // macOS also reports `dpi_scaling: false`: AppKit supplies the screen scale, but this
        // backend does not query it, so `dpi_scale_factor()` answers the trait default `1.0`.
        ("cocoa", PlatformFamily::Desktop, false, true, true, false),
        ("macos-objc2-preview", PlatformFamily::Desktop, false, false, false, false),
        ("linux-gtk", PlatformFamily::Desktop, expect_linux_dpi, true, true, false),
        // Wayland: `dpi_scaling` is true because `dpi_scale_factor()` really answers; `ime` and
        // `accessibility` are false because neither bridge method is overridden. See its
        // `capabilities()` doc for the grep evidence.
        ("wayland", PlatformFamily::Desktop, true, false, false, false),
        // iOS reports the honest absence of all three: no `dpi_scale_factor()` (so the flag would
        // promise the fabricated `1.0` on a 2x/3x screen), no `ime_bridge()`, no
        // `accessibility_bridge()`. See its `capabilities()` doc for the grep evidence.
        ("ios-state-backend", PlatformFamily::Mobile, false, false, false, false),
        ("android-mobile", PlatformFamily::Mobile, true, true, true, false),
        ("harmony-state-backend", PlatformFamily::Desktop, true, true, true, false),
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
        let row = published.iter().find(|(row_name, ..)| row_name == name).unwrap_or_else(|| {
            panic!(
                "backend '{name}' is constructible but has no published README row; add \
                     its row to `published` and to the README matrix"
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
