//! Widget Gallery Generator — renders all widgets to SVG and generates a gallery HTML page.
//!
//! Usage: cargo run --example widget_gallery [output_dir]

#[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
use rust_widgets::core::Rect;
#[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
use rust_widgets::widget::svg::render_to_svg;
#[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
use rust_widgets::widget::*;
#[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
use std::fs;
#[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
use std::path::PathBuf;

/// Gallery entry for a widget
#[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
struct GalleryEntry {
    name: &'static str,
    kind: WidgetKind,
    category: &'static str,
    render: Box<dyn Fn() -> String>,
}

fn main() {
    #[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
    {
        let out_dir = std::env::args().nth(1).unwrap_or_else(|| "widget_gallery".to_string());
        let out_path = PathBuf::from(&out_dir);
        fs::create_dir_all(&out_path).unwrap();

        let entries: Vec<GalleryEntry> = vec![
            // Base widgets
            GalleryEntry {
                name: "Button",
                kind: WidgetKind::Button,
                category: "Base",
                render: Box::new(|| {
                    let mut w = Button::new("Click Me".to_string(), Rect::new(0, 0, 120, 36));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "Label",
                kind: WidgetKind::Label,
                category: "Base",
                render: Box::new(|| {
                    let mut w = Label::new("Hello World".to_string(), Rect::new(0, 0, 200, 24));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "CheckBox",
                kind: WidgetKind::CheckBox,
                category: "Base",
                render: Box::new(|| {
                    let mut w = CheckBox::new(Rect::new(0, 0, 24, 24));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "RadioButton",
                kind: WidgetKind::RadioButton,
                category: "Base",
                render: Box::new(|| {
                    let mut w = RadioButton::new(Rect::new(0, 0, 24, 24));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "ToggleButton",
                kind: WidgetKind::ToggleButton,
                category: "Base",
                render: Box::new(|| {
                    let mut w = ToggleButton::new("Toggle".to_string(), Rect::new(0, 0, 100, 32));
                    render_to_svg(&mut w)
                }),
            },
            // Input widgets
            GalleryEntry {
                name: "LineEdit",
                kind: WidgetKind::LineEdit,
                category: "Input",
                render: Box::new(|| {
                    let mut w = LineEdit::new(Rect::new(0, 0, 200, 28));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "ComboBox",
                kind: WidgetKind::ComboBox,
                category: "Input",
                render: Box::new(|| {
                    let mut w = ComboBox::new(Rect::new(0, 0, 180, 28));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "SpinBox",
                kind: WidgetKind::SpinBox,
                category: "Input",
                render: Box::new(|| {
                    let mut w = SpinBox::new(Rect::new(0, 0, 100, 28));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "Slider",
                kind: WidgetKind::Slider,
                category: "Input",
                render: Box::new(|| {
                    let mut w = Slider::new(Rect::new(0, 0, 200, 24));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "ProgressBar",
                kind: WidgetKind::ProgressBar,
                category: "Display",
                render: Box::new(|| {
                    let mut w = ProgressBar::new(Rect::new(0, 0, 200, 24));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "ScrollBar",
                kind: WidgetKind::ScrollBar,
                category: "Display",
                render: Box::new(|| {
                    let mut w = ScrollBar::new(Rect::new(0, 0, 16, 200));
                    render_to_svg(&mut w)
                }),
            },
            // Container widgets
            GalleryEntry {
                name: "GroupBox",
                kind: WidgetKind::GroupBox,
                category: "Container",
                render: Box::new(|| {
                    let mut w = GroupBox::new(Rect::new(0, 0, 300, 200));
                    w.set_title("Group".to_string());
                    render_to_svg(&mut w)
                }),
            },
            // Modern/Switch widgets
            GalleryEntry {
                name: "Switch",
                kind: WidgetKind::Switch,
                category: "Modern",
                render: Box::new(|| {
                    let mut w = Switch::new(Rect::new(0, 0, 50, 26));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "Badge",
                kind: WidgetKind::Badge,
                category: "Modern",
                render: Box::new(|| {
                    let mut w = Badge::new(Rect::new(0, 0, 40, 20));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "Chip",
                kind: WidgetKind::Chip,
                category: "Modern",
                render: Box::new(|| {
                    let mut w = Chip::new(Rect::new(0, 0, 80, 28));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "Tooltip",
                kind: WidgetKind::Tooltip,
                category: "Modern",
                render: Box::new(|| {
                    let mut w = Tooltip::new("Tooltip text", Rect::new(0, 0, 120, 28));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "ProgressCircle",
                kind: WidgetKind::ProgressCircle,
                category: "Modern",
                render: Box::new(|| {
                    let mut w = ProgressCircle::new(Rect::new(0, 0, 48, 48));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "Icon",
                kind: WidgetKind::Icon,
                category: "Modern",
                render: Box::new(|| {
                    let mut w = Icon::new(Rect::new(0, 0, 32, 32));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "SegmentedButton",
                kind: WidgetKind::SegmentedButton,
                category: "Modern",
                render: Box::new(|| {
                    let mut w = SegmentedButton::new(Rect::new(0, 0, 240, 32));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "RangeSlider",
                kind: WidgetKind::RangeSlider,
                category: "Input",
                render: Box::new(|| {
                    let mut w = RangeSlider::new(Rect::new(0, 0, 200, 32));
                    render_to_svg(&mut w)
                }),
            },
            GalleryEntry {
                name: "FloatingLabel",
                kind: WidgetKind::FloatingLabel,
                category: "Input",
                render: Box::new(|| {
                    let mut w = FloatingLabel::new(Rect::new(0, 0, 200, 48));
                    render_to_svg(&mut w)
                }),
            },
            // Charts & Data viz
            GalleryEntry {
                name: "Sparkline",
                kind: WidgetKind::Sparkline,
                category: "Charts",
                render: Box::new(|| {
                    let mut w = Sparkline::new(Rect::new(0, 0, 100, 24));
                    render_to_svg(&mut w)
                }),
            },
        ];

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
            "2026-06-10",
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
