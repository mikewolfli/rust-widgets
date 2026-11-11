// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `rwui create` — scaffold a runnable project for one device profile.
//!
//! # What it writes
//!
//! ```text
//! <name>/
//!   Cargo.toml          the profile as an explicit feature set
//!   README.md           how to run it and where the UI lives
//!   .gitignore          target/
//!   ui/app.json         the declarative document (the design source)
//!   ui/app.css          the stylesheet with the SAME base name
//!   src/main.rs         entry stub: `app::run()` only
//!   src/app.rs          App + window + UI mount
//! ```
//!
//! # Why the UI is a JSON/CSS pair, and why the app still compiles without it
//!
//! `desktop`, `tablet` and `mobile` builds carry `serde_json`, so their `app.rs` loads
//! `ui/app.json` and registers `ui/app.css` under the same base name — the document and its
//! stylesheet are one design unit, and a rename of one that misses the other is visible in the
//! generated code (`include_str!("../ui/app.json")` next to `"../ui/app.css"`).
//!
//! `embedded` and `mini` have no `serde_json` and no theme module (principles #37/#47), so their
//! `app.rs` builds the same controls in Rust. The `ui/` pair is still written: it is the design
//! source a host can hand to `rwui json2rust` for a future regeneration, and omitting it would make
//! the two profile families describe two different projects.
//!
//! # Determinism
//!
//! Every file is a pure function of `(name, profile, rust_widgets_path)`, so running `create`
//! twice over the same arguments produces identical bytes. That is what makes a generated project
//! reviewable in a diff, and it is asserted by this module's tests.

use std::path::{Path, PathBuf};

/// The device profile a project is created for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectProfile {
    /// Full desktop: windowing, GPU, charts, print, PDF.
    Desktop,
    /// Tablet: touch plus the full widget set, no desktop-only services.
    Tablet,
    /// Mobile: touch, phone/tablet layout vocabulary.
    Mobile,
    /// Embedded: sized build, no allocator-heavy subsystems.
    Embedded,
    /// Mini: the smallest sized build.
    Mini,
}

impl ProjectProfile {
    /// Parses the spelling accepted by `--profile`.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "desktop" => Some(Self::Desktop),
            "tablet" => Some(Self::Tablet),
            "mobile" => Some(Self::Mobile),
            "embedded" => Some(Self::Embedded),
            "mini" => Some(Self::Mini),
            _ => None,
        }
    }

    /// The Cargo feature that selects this profile.
    pub fn feature(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Tablet => "tablet",
            Self::Mobile => "mobile",
            Self::Embedded => "embedded",
            Self::Mini => "mini",
        }
    }

    /// Whether this profile compiles the JSON loader (and therefore loads `ui/app.json`).
    ///
    /// The loader is gated on `full_widgets`, which is exactly "a desktop/tablet/mobile profile with
    /// an unstripped widget set" (principle #47). Asking this enum rather than testing the feature
    /// string keeps the answer in one place.
    pub fn loads_declarative_ui(self) -> bool {
        matches!(self, Self::Desktop | Self::Tablet | Self::Mobile)
    }

    /// The window size the starter asks for.
    pub fn window_size(self) -> (u32, u32) {
        match self {
            Self::Desktop => (960, 640),
            Self::Tablet => (820, 1180),
            Self::Mobile => (390, 844),
            Self::Embedded => (480, 320),
            Self::Mini => (320, 240),
        }
    }

    /// Every profile, in the order the help text lists them.
    ///
    /// Used by the template tests; the CLI's help text is a constant, so a release build has no
    /// caller and the constant must not be compiled there (dead code is not a documentation aid).
    #[cfg(test)]
    pub const ALL: [Self; 5] =
        [Self::Desktop, Self::Tablet, Self::Mobile, Self::Embedded, Self::Mini];
}

/// What to create.
#[derive(Debug, Clone)]
pub struct CreateRequest {
    /// The project (and Cargo package) name.
    pub name: String,
    /// The profile the generated `Cargo.toml` selects.
    pub profile: ProjectProfile,
    /// The directory the project directory is created under.
    pub output_root: PathBuf,
    /// A path dependency on the library, for a checkout; `None` means a version dependency.
    pub rust_widgets_path: Option<PathBuf>,
}

/// One file the command wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedFile {
    /// The absolute path written.
    pub path: PathBuf,
    /// Its size in bytes.
    pub bytes: usize,
}

/// Creates the project, returning every file written.
///
/// # Errors
///
/// Returns an error when the name is not a valid package name, the target directory already exists
/// and is not empty (a scaffold must never overwrite a project), or a file cannot be written.
pub fn create(request: &CreateRequest) -> Result<Vec<CreatedFile>, String> {
    validate_name(&request.name)?;
    let root = request.output_root.join(&request.name);
    refuse_non_empty(&root)?;

    let crate_version = env!("CARGO_PKG_VERSION");
    let files = project_files(request, crate_version);

    std::fs::create_dir_all(&root)
        .map_err(|error| format!("could not create {}: {error}", root.display()))?;

    let mut written = Vec::with_capacity(files.len());
    for (relative, contents) in files {
        let path = root.join(&relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        }
        std::fs::write(&path, contents.as_bytes())
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        written.push(CreatedFile { path, bytes: contents.len() });
    }
    Ok(written)
}

/// Rejects a name that is not a package name this template can emit verbatim.
fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("the project name must not be empty".to_string());
    }
    let mut chars = name.chars();
    let first = chars.next().expect("non-empty checked above");
    if !(first.is_ascii_lowercase() || first == '_') {
        return Err(format!(
            "project name {name:?} must start with a lowercase letter or `_` (Cargo package name)"
        ));
    }
    if let Some(bad) =
        chars.find(|c| !(*c == '_' || *c == '-' || c.is_ascii_lowercase() || c.is_ascii_digit()))
    {
        return Err(format!(
            "project name {name:?} contains {bad:?}; use lowercase ASCII letters, digits, `-` or `_`"
        ));
    }
    if name.ends_with('-') || name.ends_with('_') {
        return Err(format!("project name {name:?} must not end with `-` or `_`"));
    }
    Ok(())
}

/// Refuses to write into a directory that already holds something.
fn refuse_non_empty(root: &Path) -> Result<(), String> {
    match std::fs::read_dir(root) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                Err(format!(
                    "{} already exists and is not empty; choose another name or remove it",
                    root.display()
                ))
            } else {
                Ok(())
            }
        }
        // A missing directory is the normal case.
        Err(_) => Ok(()),
    }
}

/// Every file of the project, relative to its root.
fn project_files(request: &CreateRequest, crate_version: &str) -> Vec<(PathBuf, String)> {
    vec![
        (PathBuf::from("Cargo.toml"), manifest(request, crate_version)),
        (PathBuf::from("README.md"), readme(request)),
        (PathBuf::from(".gitignore"), String::from("/target\n")),
        (PathBuf::from("ui/app.json"), ui_document(request)),
        (PathBuf::from("ui/app.css"), ui_stylesheet()),
        (PathBuf::from("src/main.rs"), main_rs()),
        (PathBuf::from("src/app.rs"), app_rs(request)),
    ]
}

/// The generated manifest: the profile is an explicit, `default-features = false` choice.
fn manifest(request: &CreateRequest, crate_version: &str) -> String {
    let dependency = match &request.rust_widgets_path {
        Some(path) => format!(
            "rust_widgets = {{ version = \"{crate_version}\", path = {:?}, default-features = false, features = [\"{}\"] }}",
            path.display().to_string(),
            request.profile.feature()
        ),
        None => format!(
            "rust_widgets = {{ version = \"{crate_version}\", default-features = false, features = [\"{}\"] }}",
            request.profile.feature()
        ),
    };
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
rust-version = "1.88"
publish = false

[dependencies]
# The `{profile}` profile, stated explicitly. `default-features = false` is load-bearing: the
# library's default set adds `desktop` on top of any other profile, which is the mixed
# configuration principle #48 forbids.
{dependency}
"#,
        name = request.name,
        profile = request.profile.feature(),
        dependency = dependency,
    )
}

/// The README, including the exact command the profile needs.
fn readme(request: &CreateRequest) -> String {
    let (width, height) = request.profile.window_size();
    let ui_note = if request.profile.loads_declarative_ui() {
        "`ui/app.json` is loaded at run time and `ui/app.css` is registered under the same base\nname. Edit either file and rebuild; the pair is the design source."
    } else {
        "This profile has no window server and no JSON loader, so `src/app.rs` is the host\nskeleton: surface allocation, frame budget and one queued frame task. `ui/app.json` +\n`ui/app.css` are kept as the design source; hand them to `rwui json2rust` to regenerate Rust\nfor a declarative build."
    };
    format!(
        "# {name}\n\n\
         A rust_widgets application for the `{profile}` profile ({width}x{height}).\n\n\
         ## Run\n\n\
         ```text\ncargo run\n```\n\n\
         ## Layout\n\n\
         - `src/main.rs` — entry stub; it only calls `app::run()`.\n\
         - `src/app.rs` — creates the App and the window, then mounts the UI.\n\
         - `ui/app.json` + `ui/app.css` — the UI document and its stylesheet.\n\n\
         {ui_note}\n\n\
         ## Change the profile\n\n\
         The profile is one feature in `Cargo.toml`. Switching it means changing that feature and\n\
         rebuilding; never list two device profiles at once (they are mutually exclusive).\n",
        name = request.name,
        profile = request.profile.feature(),
        width = width,
        height = height,
        ui_note = ui_note,
    )
}

/// The declarative document. A `panel` root is used because the document describes the **content**
/// of the window: the platform window is created by the host, and its root is parented into it.
fn ui_document(request: &CreateRequest) -> String {
    let (_, height) = request.profile.window_size();
    // The label and button coordinates are solved by the document's own vertical layout, so the
    // numbers here are only the initial rects a control is constructed with.
    let button_top = (height / 2).saturating_sub(20);
    format!(
        r##"{{
  "panel": {{
    "id": "root",
    "layout": {{
      "type": "vbox",
      "spacing": 12,
      "margin": 24,
      "children": [
        {{ "label": {{ "id": "title", "text": "Hello, rust_widgets" }} }},
        {{ "label": {{ "id": "hint", "text": "Edit ui/app.json and ui/app.css, then rebuild." }} }},
        {{ "button": {{ "id": "primary", "text": "Click me", "x": 24, "y": {button_top}, "width": 140, "height": 36 }} }}
      ]
    }}
  }}
}}
"##,
        button_top = button_top,
    )
}

/// The stylesheet with the same base name as the document.
fn ui_stylesheet() -> String {
    String::from(
        "/* ui/app.css — same base name as ui/app.json; both are one design unit. */\n\
         #title {\n    color: #f5f5f7;\n}\n\n\
         #hint {\n    color: #9a9aa2;\n}\n",
    )
}

/// The entry stub (project convention: `main` only calls `app::run()`).
fn main_rs() -> String {
    String::from(
        "// SPDX-FileCopyrightText: Copyright (c) 2026 the project authors\n\
         // SPDX-License-Identifier: MIT\n\n\
         //! Entry point. Everything else lives in [`app`].\n\n\
         mod app;\n\n\
         fn main() {\n    app::run();\n}\n",
    )
}

/// `src/app.rs` for the profile.
fn app_rs(request: &CreateRequest) -> String {
    if request.profile.loads_declarative_ui() {
        declarative_app_rs(request)
    } else {
        rust_app_rs(request)
    }
}

fn declarative_app_rs(request: &CreateRequest) -> String {
    let (width, height) = request.profile.window_size();
    format!(
        r#"// SPDX-FileCopyrightText: Copyright (c) 2026 the project authors
// SPDX-License-Identifier: MIT

//! Window creation and UI mounting.
//!
//! The UI is a **pair** of same-named files under `ui/`: the document (`app.json`) and its
//! stylesheet (`app.css`). Both are embedded at compile time, so the binary runs from any working
//! directory. The document describes the content of the window; the platform window itself is
//! created here by the host, and the document's root control is parented into it.

use rust_widgets::app::{{App, AppConfig}};
use rust_widgets::core::Rect;

/// Application (and window) title.
const TITLE: &str = "{title}";
/// Client area the starter asks for.
const WIDTH: u32 = {width};
const HEIGHT: u32 = {height};

const UI_JSON: &str = include_str!("../ui/app.json");
const UI_CSS: &str = include_str!("../ui/app.css");

/// Runs the application.
pub fn run() {{
    let mut app = App::with_config(
        AppConfig::default()
            .with_app_name(TITLE)
            .with_organization("{name}"),
    );
    app.init();

    // The stylesheet is registered under the document's base name so the two cannot drift apart.
    rust_widgets::style::global_stylesheet_manager().register("app", UI_CSS, 0);

    let window = app.new_window(TITLE, 80, 80, WIDTH, HEIGHT);
    if window.raw_id() == 0 {{
        eprintln!("this backend cannot create a window; the UI was not shown");
        return;
    }}

    match mount_ui(window.raw_id()) {{
        Ok(()) => app.run(),
        Err(error) => eprintln!("the UI could not be loaded: {{error}}"),
    }}
}}

/// Builds the document and attaches its root to `window_id`.
///
/// Run this **once per process**: the loader mounts every control under the document's `id`s and
/// ids live for the process lifetime, so loading the same document again is refused as a duplicate
/// (the refusal is reported, not silent). For an edit-reload loop use `JsonLoader::session()`, which
/// patches the mounted tree instead of mounting a second one.
fn mount_ui(window_id: rust_widgets::core::ObjectId) -> Result<(), String> {{
    let document = rust_widgets::json::JsonLoader::load(UI_JSON)?;
    // The document declares the root's `id`, and that name is how every other front end addresses
    // a node — so the starter names it rather than relying on registration order.
    let root = document
        .id("root")
        .ok_or_else(|| String::from("the document declared no `root` control"))?;

    // Reparenting is what makes the declarative tree part of the window: the loader creates and
    // links the controls, but a document cannot create the platform window it is shown in.
    let attached = rust_widgets::widget::runtime::with_widget_mut(window_id, |window| {{
        window.base_mut().add_child_linked(root)
    }})
    .unwrap_or(false);
    if !attached {{
        return Err(String::from("the window refused the document's root control"));
    }}

    let client = Rect::new(0, 0, WIDTH, HEIGHT);
    rust_widgets::widget::runtime::set_geometry(root, client);
    // The document's own layout places its children inside the root.
    let _ = rust_widgets::json::apply_layout(root, client);

    // A finished wiring is a repaint request: the window may already have painted an empty client
    // area before the tree existed.
    rust_widgets::widget::runtime::request_repaint(window_id);
    Ok(())
}}
"#,
        title = request.name.replace('-', " "),
        width = width,
        height = height,
        name = request.name,
    )
}

fn rust_app_rs(request: &CreateRequest) -> String {
    let (width, height) = request.profile.window_size();
    format!(
        r#"// SPDX-FileCopyrightText: Copyright (c) 2026 the project authors
// SPDX-License-Identifier: MIT

//! Host skeleton for a sized profile.
//!
//! `embedded` and `mini` have no window server and no `serde_json` (principles #37/#47), so this
//! file is the **integration point** rather than a window UI: it asks the build what it is,
//! configures the frame budget, allocates a surface with an explicit stride, queues one frame task
//! and reports the engine's own counters. Wire your board's loop to the same engine; the UI
//! document in `ui/app.json` is the design source a declarative build consumes.

use rust_widgets::platform::portable::copy_rows;
use rust_widgets::platform::profile;
use rust_widgets::platform::{{FrameBuffer, SurfaceGeometry}};

/// Frame size the starter asks for.
const WIDTH: u32 = {width};
const HEIGHT: u32 = {height};
/// Frames the starter runs before returning.
const FRAMES: u32 = 60;

/// Runs the host.
pub fn run() {{
    println!("profile={{}}", profile::profile_name());
    println!("os_runtime={{}}", profile::has_os_runtime());
    println!("alloc_frugal={{}}", profile::is_alloc_frugal());
    println!("supports_surfaces={{}}", rust_widgets::supports_surfaces());

    let applied_fps = rust_widgets::render_engine::set_embedded_target_fps(60);
    println!("fps={{applied_fps}}");

    // A padded stride is what a real panel uses; `width * 4` would shear it.
    let geometry = SurfaceGeometry {{ width: WIDTH, height: HEIGHT, stride: WIDTH as usize * 4 }};
    let mut frame = FrameBuffer::new();
    if !frame.resize(geometry) {{
        eprintln!("the surface geometry was rejected");
        return;
    }}
    println!("surface_bytes={{}}", frame.frame().len());

    // One task on the next engine frame. A repeating task reschedules itself the same way.
    let task = rust_widgets::render_engine::submit_embedded_task("startup", |frame_index| {{
        println!("task_ran_on_frame={{frame_index}}");
    }});
    println!("task_id={{task}}");

    // The board's loop replaces this bounded loop; it is here so a desktop `cargo run` of the
    // skeleton terminates instead of spinning.
    for _ in 0..FRAMES {{
        let stats = rust_widgets::render_engine::embedded_engine_stats();
        if stats.frame_count >= u64::from(FRAMES) {{
            break;
        }}
    }}

    let stats = rust_widgets::render_engine::embedded_engine_stats();
    println!("frames={{}}", stats.frame_count);
    println!("pending_tasks={{}}", stats.pending_task_count);

    // `copy_rows` is the portable host's reshape service for a foreign buffer; kept here so the
    // integration point names the call a real driver makes.
    let rows = geometry.stride * HEIGHT as usize;
    let mut destination = vec![0u8; rows];
    println!("reshape_ok={{}}", copy_rows(frame.frame(), &mut destination, geometry));
}}
"#,
        width = width,
        height = height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(profile: ProjectProfile) -> CreateRequest {
        // A per-call suffix: the harness runs these tests in parallel, and a shared directory per
        // profile let one test's cleanup delete another test's project mid-assertion.
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        let request = CreateRequest {
            name: "demo-app".to_string(),
            profile,
            output_root: std::env::temp_dir().join(format!(
                "rwui-create-{}-{}-{id}",
                profile.feature(),
                std::process::id()
            )),
            rust_widgets_path: None,
        };
        let _ = std::fs::remove_dir_all(&request.output_root);
        request
    }

    #[test]
    fn every_profile_writes_the_same_set_of_files() {
        for profile in ProjectProfile::ALL {
            let request = scratch(profile);
            let files = create(&request).expect("create");
            let names: Vec<String> = files
                .iter()
                .map(|file| {
                    file.path
                        .strip_prefix(request.output_root.join(&request.name))
                        .expect("under the project root")
                        .to_string_lossy()
                        .replace('\\', "/")
                })
                .collect();
            assert_eq!(
                names,
                vec![
                    "Cargo.toml",
                    "README.md",
                    ".gitignore",
                    "ui/app.json",
                    "ui/app.css",
                    "src/main.rs",
                    "src/app.rs"
                ],
                "profile {} wrote an unexpected file set",
                profile.feature()
            );
            let _ = std::fs::remove_dir_all(&request.output_root);
        }
    }

    #[test]
    fn the_manifest_states_exactly_one_profile() {
        for profile in ProjectProfile::ALL {
            let request = scratch(profile);
            create(&request).expect("create");
            let manifest =
                std::fs::read_to_string(request.output_root.join(&request.name).join("Cargo.toml"))
                    .expect("manifest");
            assert!(
                manifest.contains(&format!("features = [\"{}\"]", profile.feature())),
                "the manifest must select {}",
                profile.feature()
            );
            assert!(
                manifest.contains("default-features = false"),
                "a mixed profile is the configuration #48 forbids"
            );
            for other in ProjectProfile::ALL {
                if other == profile {
                    continue;
                }
                assert!(
                    !manifest.contains(&format!("\"{}\"", other.feature())),
                    "{} must not also name {}",
                    profile.feature(),
                    other.feature()
                );
            }
            let _ = std::fs::remove_dir_all(&request.output_root);
        }
    }

    #[test]
    fn the_declarative_profiles_load_the_ui_pair_and_the_sized_profiles_build_in_rust() {
        for profile in ProjectProfile::ALL {
            let request = scratch(profile);
            create(&request).expect("create");
            let app =
                std::fs::read_to_string(request.output_root.join(&request.name).join("src/app.rs"))
                    .expect("app.rs");
            if profile.loads_declarative_ui() {
                assert!(
                    app.contains("include_str!(\"../ui/app.json\")"),
                    "{} must embed the document",
                    profile.feature()
                );
                assert!(
                    app.contains("include_str!(\"../ui/app.css\")"),
                    "{} must embed the stylesheet",
                    profile.feature()
                );
                assert!(
                    app.contains("JsonLoader::load"),
                    "{} must load the document",
                    profile.feature()
                );
            } else {
                assert!(!app.contains("JsonLoader"), "{} has no loader", profile.feature());
                assert!(
                    app.contains("submit_embedded_task") && app.contains("FrameBuffer"),
                    "{} must be the sized-profile host skeleton",
                    profile.feature()
                );
                assert!(
                    !app.contains("include_str!"),
                    "{} must not embed a document it cannot parse",
                    profile.feature()
                );
            }
            let _ = std::fs::remove_dir_all(&request.output_root);
        }
    }

    #[test]
    fn the_document_parses_and_its_stylesheet_shares_its_name() {
        for profile in ProjectProfile::ALL {
            let request = scratch(profile);
            create(&request).expect("create");
            let json = std::fs::read_to_string(
                request.output_root.join(&request.name).join("ui/app.json"),
            )
            .expect("document");
            let value: serde_json::Value =
                serde_json::from_str(&json).expect("the document must parse");
            assert!(value.get("panel").is_some(), "the template's root is a panel");
            // The document is the file the generated `app.rs` feeds to the loader. Loading it
            // through the library's own loader is the end-to-end check. It runs **once**: the
            // loader mounts controls under the document's `id`s, and ids are process-lifetime, so a
            // second load of the same document in one thread is (correctly) refused as a duplicate.
            if profile == ProjectProfile::Desktop {
                let loaded = rust_widgets::json::JsonLoader::load(&json)
                    .expect("the template document must load through the library's own loader");
                let root_id = loaded.id("root").expect("the template declares id root");
                assert!(root_id != 0, "the loaded document must have a mounted `root` control");
            }
            let css =
                std::fs::read_to_string(request.output_root.join(&request.name).join("ui/app.css"))
                    .expect("stylesheet");
            assert!(css.contains("#title"), "the stylesheet must target the declared id");
            let _ = std::fs::remove_dir_all(&request.output_root);
        }
    }

    #[test]
    fn an_existing_non_empty_directory_is_refused() {
        let request = scratch(ProjectProfile::Desktop);
        let root = request.output_root.join(&request.name);
        std::fs::create_dir_all(&root).expect("dir");
        std::fs::write(root.join("keep.txt"), "user work").expect("write");
        let error = create(&request).expect_err("a non-empty target must be refused");
        assert!(error.contains("not empty"), "the reason must be stated: {error}");
        assert!(root.join("keep.txt").exists(), "the refusal must not destroy what was there");
        let _ = std::fs::remove_dir_all(&request.output_root);
    }

    #[test]
    fn a_name_that_is_not_a_package_name_is_refused() {
        for bad in ["", "1abc", "MyApp", "app name", "app/name", "app-"] {
            let mut request = scratch(ProjectProfile::Desktop);
            request.name = bad.to_string();
            assert!(create(&request).is_err(), "{bad:?} must be refused");
            let _ = std::fs::remove_dir_all(&request.output_root);
        }
    }

    #[test]
    fn creation_is_deterministic() {
        let first = scratch(ProjectProfile::Tablet);
        let files = create(&first).expect("create");
        let snapshot: Vec<(String, String)> = files
            .iter()
            .map(|file| {
                (
                    file.path.display().to_string(),
                    std::fs::read_to_string(&file.path).expect("read"),
                )
            })
            .collect();
        let _ = std::fs::remove_dir_all(&first.output_root);

        let second = scratch(ProjectProfile::Tablet);
        create(&second).expect("create again");
        for (path, contents) in snapshot {
            let relative = path.rsplit('/').next().expect("file name");
            let file = find_by_name(&second, relative).expect("same file exists");
            assert_eq!(
                std::fs::read_to_string(&file).expect("read"),
                contents,
                "{relative} drifted"
            );
        }
        let _ = std::fs::remove_dir_all(&second.output_root);
    }

    fn find_by_name(request: &CreateRequest, name: &str) -> Option<PathBuf> {
        let root = request.output_root.join(&request.name);
        for relative in [
            "Cargo.toml",
            "README.md",
            ".gitignore",
            "src/main.rs",
            "src/app.rs",
            "ui/app.json",
            "ui/app.css",
        ] {
            if relative.ends_with(name) {
                return Some(root.join(relative));
            }
        }
        None
    }
}
