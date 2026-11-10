//! Widget Gallery Generator — renders every published control to SVG and generates a gallery page.
//!
//! Usage: cargo run --example widget_gallery [output_dir]
//!
//! # Why the set comes from the factory (GX2-2)
//!
//! This used to hold 24 hand-written entries out of the registry's published set, with six
//! hand-chosen category names, so the page could silently describe a quarter of the library. The
//! sweep now asks [`WidgetFactory::widget_names`] — the same source the rendering census uses,
//! which cannot under-count — and groups each control by [`WidgetKind::designer_category`], the
//! palette category its capability manifest already carries. A control that is registered but
//! cannot be constructed is reported on stderr rather than dropped, because that is itself a
//! finding about the registry.
//!
//! # Why the date is generated
//!
//! The page used to claim a hard-coded `2026-06-10` regardless of when it ran. It now prints the
//! UTC date the run happened on, computed from `SystemTime` (no dependency and no locale).

#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]
use rust_widgets::core::Rect;
#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]
use rust_widgets::widget::{
    draw_bridge, svg::render_widget_to_svg, AdaptiveScaffold, MdiArea, WidgetFactory, WidgetKind,
};
#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]
use std::fs;
#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]
use std::path::PathBuf;

/// The box every control is rendered into.
#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]
const GALLERY_RECT: Rect = Rect::new(0, 0, 240, 160);

/// The text every control is rendered with, so a caption is not the empty string.
#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]
const GALLERY_TEXT: &str = "Sample";

/// Gallery entry for a widget
#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]
struct GalleryEntry {
    name: &'static str,
    kind: WidgetKind,
    category: &'static str,
    render: Box<dyn Fn() -> String>,
}

/// The UTC date of this run, as `YYYY-MM-DD`.
///
/// One small civil-date conversion rather than a dependency: the worst answer for a machine whose
/// clock predates the epoch is `1970-01-01`, which is honest and impossible to mistake for a
/// hard-coded claim.
#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]
fn generated_on() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Howard Hinnant's `civil_from_days`, the standard branch-free inverse of the day-count
/// algorithm (the same one the file dialog and the date editor carry).
#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = (if month_prime < 10 { month_prime + 3 } else { month_prime - 9 }) as u32;
    year += i64::from(month <= 2);
    (year, month, day)
}

/// Renders a freshly constructed control to SVG, themed and with sample content.
#[cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]
fn render_control(name: &str) -> String {
    let Some(mut widget) = WidgetFactory::shared().create(name, GALLERY_RECT, GALLERY_TEXT) else {
        return String::new();
    };
    // Content first: a control's default state renders empty otherwise (the same fill the
    // rendering census applies). The gallery draws every control in the library's default style;
    // applying an appearance per control is what the census does, and it is a crate-internal step.
    rust_widgets::widget::sample_fill::apply(name, widget.as_mut());
    match draw_bridge::draw_of(widget.as_mut()) {
        Some(drawable) => render_widget_to_svg(drawable, GALLERY_RECT),
        None => String::new(),
    }
}

fn main() {
    #[cfg(all(
        any(feature = "desktop", feature = "tablet", feature = "mobile"),
        not(any(feature = "mini", feature = "embedded"))
    ))]
    {
        // A directory path needs no UTF-8 decoding: `args_os()` yields it as an `OsString`, so a
        // name with an unusual byte is used as-is rather than panicking (DM6-04).
        let out_path = std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("widget_gallery"));
        fs::create_dir_all(&out_path).unwrap();

        // Every published control, exactly once: the factory's own name list is the authority.
        let factory = WidgetFactory::shared();
        let mut entries: Vec<GalleryEntry> = Vec::new();
        for name in factory.widget_names() {
            let Some(widget) = factory.create(name, GALLERY_RECT, GALLERY_TEXT) else {
                eprintln!("widget_gallery: {name} is registered but could not be constructed");
                continue;
            };
            let kind = widget.kind();
            entries.push(GalleryEntry {
                name,
                kind,
                category: kind.designer_category(),
                render: Box::new(move || render_control(name)),
            });
        }

        // GX2-1 / GX2-2: the two composition paradigms get a section of their own. They are not
        // single controls with a settable size hint, so they are constructed explicitly rather
        // than through the factory.
        entries.push(GalleryEntry {
            name: "AdaptiveScaffold",
            kind: WidgetKind::AdaptiveScaffold,
            category: "patterns",
            render: Box::new(|| {
                let mut scaffold = AdaptiveScaffold::new("Scaffold", GALLERY_RECT);
                scaffold.set_show_bottom_nav(true);
                scaffold.add_nav_item("home", "Home");
                scaffold.add_nav_item("search", "Search");
                scaffold.add_nav_item("settings", "Settings");
                render_widget_to_svg(&mut scaffold, GALLERY_RECT)
            }),
        });
        entries.push(GalleryEntry {
            name: "MdiArea",
            kind: WidgetKind::MdiArea,
            category: "patterns",
            render: Box::new(|| {
                let mut area = MdiArea::new(GALLERY_RECT);
                area.add_sub_window(1, Rect::new(8, 8, 150, 90));
                area.add_sub_window(2, Rect::new(60, 40, 150, 90));
                area.set_sub_window_title(1, "Project A".to_string());
                area.set_sub_window_title(2, "Project B".to_string());
                render_widget_to_svg(&mut area, GALLERY_RECT)
            }),
        });

        // Generate SVG for each widget
        let mut gallery_html = String::new();
        gallery_html.push_str("<html><head><title>Widget Gallery</title>");
        gallery_html.push_str("<style>");
        gallery_html.push_str("body{font-family:Arial,sans-serif;margin:20px;background:#f5f5f5}");
        gallery_html.push_str("h1{color:#333}");
        gallery_html.push_str("h2{color:#555;border-bottom:2px solid #ddd;padding-bottom:5px}");
        gallery_html.push_str(".category{margin-bottom:30px}");
        gallery_html.push_str(
            ".widget-card{display:inline-block;margin:10px;padding:15px;background:#fff;\
             border:1px solid #e0e0e0;border-radius:8px;text-align:center;\
             box-shadow:0 2px 4px rgba(0,0,0,0.06)}",
        );
        gallery_html.push_str(".widget-card img{display:block;margin:0 auto}");
        gallery_html
            .push_str(".widget-name{font-size:13px;margin-top:8px;color:#666;font-weight:500}");
        gallery_html.push_str(".widget-kind{font-size:11px;color:#999}");
        gallery_html.push_str("</style></head><body>");
        gallery_html.push_str("<h1>Widget Gallery</h1>");
        gallery_html.push_str(&format!(
            "<p>Generated: {} | Total widgets: {}</p>",
            generated_on(),
            entries.len()
        ));

        let mut categories: std::collections::BTreeMap<&str, Vec<&GalleryEntry>> =
            std::collections::BTreeMap::new();
        for entry in &entries {
            categories.entry(entry.category).or_default().push(entry);
        }

        for (category, widgets) in &categories {
            gallery_html.push_str(&format!("<div class='category'><h2>{}</h2>", category));
            for entry in widgets {
                let svg = (entry.render)();
                let filename = format!(
                    "{}_{}.svg",
                    entry.name.to_lowercase().replace(' ', "_"),
                    entry.kind as u32
                );
                fs::write(out_path.join(&filename), &svg).unwrap();

                gallery_html.push_str(&format!(
                    "<div class='widget-card'>\
                     <img src='{}' width='200' height='auto'/>\
                     <div class='widget-name'>{}</div>\
                     <div class='widget-kind'>{:?}</div>\
                     </div>",
                    filename, entry.name, entry.kind
                ));
            }
            gallery_html.push_str("</div>");
        }

        gallery_html.push_str("</body></html>");
        fs::write(out_path.join("index.html"), gallery_html).unwrap();
        println!("Gallery generated in: {:?}", out_path);
        println!("Total widgets: {}", entries.len());
    }
}
