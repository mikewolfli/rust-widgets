# PDF & Printing

rust-widgets provides PDF document generation with drawing operators,
annotations, form fields, hyperlinks, metadata, and an SVG-based export
pipeline — plus a print system with pagination, platform backend selection,
and print dialogs.

---

## 1. PDF Generation Overview

The PDF module provides:

- **`PdfDocument` trait**: Create, modify, and save multi-page PDFs
- **`PdfPage` trait**: Draw text, lines, rectangles, and images on pages
- **`PdfWriter`**: Convenience factory for creating documents
- **Annotations**: 29 annotation types with custom properties
- **Form fields**: 8 field types with full serialization
- **Hyperlinks**: URI, page navigation, and named actions
- **Metadata & Security**: Document properties and diagnostic security model
- **SVG Export Pipeline**: `PdfExporter` for widget-to-PDF rendering

---

## 2. `PdfDocument` Trait

```rust
pub trait PdfDocument {
    fn page_count(&self) -> u32;
    fn get_page(&mut self, index: u32) -> Option<&mut dyn PdfPage>;
    fn add_page(&mut self, size: Size) -> u32;
    fn insert_page(&mut self, index: u32, size: Size) -> u32;
    fn remove_page(&mut self, index: u32) -> bool;
    fn reorder_pages(&mut self, new_order: &[u32]) -> bool;
    fn metadata(&self) -> &PdfMetadata;
    fn set_metadata(&mut self, metadata: PdfMetadata);
    fn security(&self) -> &PdfSecurity;
    fn set_security(&mut self, security: PdfSecurity);
    fn set_page_numbering_enabled(&mut self, enabled: bool);
    fn set_page_numbering_format(&mut self, prefix: &str, start_at: u32);
    fn set_page_numbering_layout(&mut self, right_margin: f32, bottom_margin: f32, font_size: f32);
    fn save(&self, path: &str) -> Result<(), std::io::Error>;
    fn to_bytes(&self) -> Result<Vec<u8>, std::io::Error>;
}
```

### Creating a Document with `PdfWriter`

```rust
use rust_widgets::pdf::PdfWriter;
use rust_widgets::core::Size;

// Create a writer (detects platform backend for print integration)
let writer = PdfWriter::new();
println!("PDF backend: {}", writer.backend_name());

// Create an A4 document
let mut doc = writer.create_document(Size { width: 595.0, height: 842.0 });

// Or create with an embedded font
let mut doc = writer.create_document_with_font_path(
    Size { width: 595.0, height: 842.0 },
    "CustomFont",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
).expect("Failed to load font");
```

### Page Management

```rust
// Add pages
let page1 = doc.add_page(Size { width: 595.0, height: 842.0 });
let page2 = doc.add_page(Size { width: 595.0, height: 842.0 });

// Insert page at index
doc.insert_page(1, Size { width: 595.0, height: 842.0 });

// Remove a page (keeps at least one page)
doc.remove_page(0);

// Reorder pages
doc.reorder_pages(&[2, 0, 1]);

println!("Page count: {}", doc.page_count());
```

### Page Numbering

```rust
doc.set_page_numbering_enabled(true);
doc.set_page_numbering_format("Page", 1);
doc.set_page_numbering_layout(20.0, 20.0, 10.0);
// Produces footer: "Page 1", "Page 2", etc.
```

### Saving

```rust
// Save to file
doc.save("output.pdf").expect("Failed to save PDF");

// Get raw bytes (for network upload, embedding, etc.)
let bytes = doc.to_bytes().expect("Failed to generate PDF bytes");
std::fs::write("output.pdf", &bytes).unwrap();
```

---

## 3. `PdfPage` Trait — Drawing Operators

```rust
pub trait PdfPage {
    fn size(&self) -> Size;
    fn set_size(&mut self, size: Size);
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: Color);
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color);
    fn draw_rect(&mut self, rect: Rect, width: f32, color: Color);
    fn fill_rect(&mut self, rect: Rect, color: Color);
    fn draw_image(&mut self, image: &[u8], rect: Rect);
    fn add_text_field(&mut self, name: &str, rect: Rect, default_text: &str);
    fn add_checkbox(&mut self, name: &str, rect: Rect, checked: bool);
    fn add_button(&mut self, name: &str, rect: Rect, text: &str);
    fn content(&self) -> Vec<u8>;
    fn form_fields(&self) -> Vec<PdfFormField>;
}
```

### Complete Drawing Example

```rust
use rust_widgets::pdf::PdfWriter;
use rust_widgets::core::{Color, Rect, Size};

let writer = PdfWriter::new();
let mut doc = writer.create_document(Size { width: 595.0, height: 842.0 });

// Get the first page
if let Some(page) = doc.get_page(0) {
    // Draw text at position (PDF coordinates have y=0 at bottom)
    page.draw_text(
        "Invoice #12345",
        50.0, 750.0,       // x, y in points
        24.0,               // font size
        Color { r: 33, g: 33, b: 33, a: 255 },
    );

    // Subtitle
    page.draw_text(
        "Date: 2026-06-10",
        50.0, 720.0,
        12.0,
        Color { r: 100, g: 100, b: 100, a: 255 },
    );

    // Separator line
    page.draw_line(
        50.0, 700.0,        // start
        545.0, 700.0,       // end
        1.0,                // width
        Color { r: 200, g: 200, b: 200, a: 255 },
    );

    // Filled header bar
    page.fill_rect(
        Rect::new(50, 650, 495, 30),
        Color { r: 66, g: 133, b: 244, a: 255 },
    );

    page.draw_text(
        "Item",
        60.0, 658.0,
        12.0,
        Color { r: 255, g: 255, b: 255, a: 255 },
    );

    page.draw_text(
        "Qty",
        300.0, 658.0,
        12.0,
        Color { r: 255, g: 255, b: 255, a: 255 },
    );

    page.draw_text(
        "Price",
        400.0, 658.0,
        12.0,
        Color { r: 255, g: 255, b: 255, a: 255 },
    );

    // Bordered rectangle
    page.draw_rect(
        Rect::new(50, 550, 495, 100),
        2.0,
        Color { r: 100, g: 100, b: 100, a: 255 },
    );

    // Draw image (PNG/JPEG raw bytes)
    let image_bytes = std::fs::read("logo.png").unwrap_or_default();
    page.draw_image(&image_bytes, Rect::new(400, 720, 145, 60));
}

doc.save("invoice.pdf").expect("Failed to save");
```

### Coordinate System

PDF uses a bottom-left origin. The `to_pdf_y()` function converts top-left
coordinates to PDF coordinates:

```rust
// y=0 at top (widget coords) → y=842 at bottom (PDF coords)
let pdf_y = page_size.height - y;
```

---

## 4. Annotations (29 Types)

### `AnnotationType` Enum

| # | Type | Description |
|---|------|-------------|
| 1 | `Text` | Sticky note / text annotation |
| 2 | `Highlight` | Text highlight markup |
| 3 | `Underline` | Text underline markup |
| 4 | `StrikeOut` | Strikethrough markup |
| 5 | `Squiggly` | Squiggly underline |
| 6 | `Link` | Hyperlink annotation |
| 7 | `Popup` | Pop-up window for markup |
| 8 | `Line` | Line annotation |
| 9 | `Square` | Rectangle annotation |
| 10 | `Circle` | Circle/ellipse annotation |
| 11 | `Polygon` | Closed polygon |
| 12 | `PolyLine` | Open polyline |
| 13 | `Ink` | Freehand "pencil" annotation |
| 14 | `Stamp` | Rubber stamp |
| 15 | `Caret` | Caret (insertion point) |
| 16 | `FileAttachment` | Embedded file |
| 17 | `Sound` | Audio annotation |
| 18 | `Movie` | Movie annotation |
| 19 | `Widget` | Form field widget |
| 20 | `Screen` | Screen annotation |
| 21 | `PrinterMark` | Printer's mark |
| 22 | `TrapNet` | Trap network |
| 23 | `Watermark` | Watermark |
| 24 | `ThreeD` | 3D annotation |
| 25–29 | *(reserved)* | Future expansion |

### Creating Annotations

```rust
use rust_widgets::pdf::annotation::{Annotation, AnnotationType, AnnotationFlags};
use rust_widgets::core::{Color, Rect};

let annotation = Annotation::new(
    "ann-1".into(),     // id
    0,                   // page index
    AnnotationType::Highlight,
    Rect::new(50, 400, 200, 20),
)
.with_contents("Important: review this section".into())
.with_author("John Doe".into())
.with_color(Color { r: 255, g: 255, b: 0, a: 128 })
.with_opacity(0.5);

// Check visibility
if annotation.is_visible() {
    println!("Annotation is visible");
}
```

### `AnnotationFlags`

```rust
pub struct AnnotationFlags {
    pub hidden: bool,        // Do not display
    pub invisible: bool,     // Do not display if not hovered
    pub locked: bool,        // Cannot be deleted
    pub locked_contents: bool, // Contents cannot be modified
    pub print: bool,         // Print the annotation
    pub no_zoom: bool,       // Do not scale with zoom
    pub no_rotate: bool,     // Do not rotate with page
    pub no_view: bool,       // Do not display on screen
    pub read_only: bool,     // Cannot interact with
    pub toggle_no_view: bool, // Toggle visibility behavior
}
```

---

## 5. Form Fields (8 Types)

### `FieldType` Enum

| # | Type | Description |
|---|------|-------------|
| 1 | `Text` | Single-line or multi-line text input |
| 2 | `Checkbox` | Binary checkbox |
| 3 | `Radio` | Radio button group |
| 4 | `ListBox` | Scrollable list selection |
| 5 | `ComboBox` | Drop-down selection |
| 6 | `Button` | Push button |
| 7 | `Signature` | Digital signature field |
| 8 | *(reserved)* | |

### `FormField` Struct

```rust
use rust_widgets::pdf::form::{FormField, FieldType};
use rust_widgets::core::{Color, Rect};

let field = FormField::new(
    "field-name".into(),    // id
    "CustomerName".into(),  // name
    FieldType::Text,
    0,                      // page
    Rect::new(100, 500, 300, 20),
);

// Configure with builder-style methods
field.with_value("John Smith".into())
    .with_default_value("Enter name".into())
    .with_font_size(12.0)
    .with_text_color(Color { r: 0, g: 0, b: 0, a: 255 })
    .with_background_color(Color { r: 255, g: 255, b: 255, a: 255 })
    .with_border(Color { r: 150, g: 150, b: 150, a: 255 }, 1.0)
    .with_tooltip("Enter your full name".into())
    .with_max_length(100)
    .with_multiline(false)
    .with_password(false)
    .with_read_only(false)
    .with_required(true);
```

### Form Field Properties

| Property | Type | Description |
|----------|------|-------------|
| `id` | `String` | Unique identifier |
| `name` | `String` | Field name (form data key) |
| `field_type` | `FieldType` | One of 8 types |
| `page` | `u32` | Page index |
| `rect` | `Rect` | Bounding rectangle |
| `value` | `String` | Current value |
| `default_value` | `String` | Default value |
| `is_read_only` | `bool` | Prevents editing |
| `is_required` | `bool` | Required for submission |
| `is_hidden` | `bool` | Hidden field |
| `tooltip` | `String` | Hover tooltip |
| `font_name` | `String` | Font family (default: "Helvetica") |
| `font_size` | `f32` | Font size (default: 12.0) |
| `text_color` | `Color` | Text color |
| `background_color` | `Option<Color>` | Background fill |
| `border_color` | `Option<Color>` | Border color |
| `border_width` | `f32` | Border width |
| `options` | `Vec<String>` | List/Combo options |
| `max_length` | `Option<u32>` | Maximum text length |
| `is_multiline` | `bool` | Multi-line text |
| `is_password` | `bool` | Password masking |
| `is_spell_check_enabled` | `bool` | Spell checking |
| `is_scrollable` | `bool` | Scrollable text area |

### Adding Form Fields via PdfPage

```rust
if let Some(page) = doc.get_page(0) {
    // Text field
    page.add_text_field("email", Rect::new(100, 700, 200, 20), "user@example.com");

    // Checkbox
    page.add_checkbox("subscribe", Rect::new(100, 670, 15, 15), true);

    // Button
    page.add_button("submit", Rect::new(100, 620, 120, 30), "Submit");

    // Retrieve all form fields
    let fields = page.form_fields();
    for field in &fields {
        println!("Field: {}", field.name);
    }
}
```

### `PdfFormField` Enum (Serialized)

```rust
pub enum PdfFormField {
    TextField { name: String, rect: Rect, value: String },
    CheckBox { name: String, rect: Rect, checked: bool },
    Button { name: String, rect: Rect, text: String },
    // ... additional variants for radio, list, combo, signature
}
```

---

## 6. Hyperlinks

### `LinkAction` Enum

```rust
pub enum LinkAction {
    GoToPage { page: u32, x: f32, y: f32 },          // Navigate to page
    GoToNamedDestination(String),                      // Named destination
    Uri(String),                                       // External URL
    LaunchFile(String),                                // Launch external file
    JavaScript(String),                                // Execute JS action
    NamedAction(NamedAction),                          // Standard action
}

pub enum NamedAction {
    NextPage, PrevPage, FirstPage, LastPage,
    Print, SaveAs,
}
```

### Creating Hyperlinks

```rust
use rust_widgets::pdf::hyperlink::{Hyperlink, LinkAction, HighlightMode, NamedDestination};
use rust_widgets::core::Rect;

// URI hyperlink
let link = Hyperlink::new(
    "link-1".into(),
    0,
    Rect::new(100, 500, 200, 20),
    LinkAction::Uri("https://example.com".into()),
)
.with_tooltip("Visit our website".into());

// Page navigation
let link = Hyperlink::new(
    "link-2".into(),
    0,
    Rect::new(100, 450, 200, 20),
    LinkAction::GoToPage { page: 3, x: 0.0, y: 0.0 },
)
.with_tooltip("Go to page 3".into());

// Named action
let link = Hyperlink::new(
    "link-3".into(),
    0,
    Rect::new(400, 30, 100, 20),
    LinkAction::NamedAction(NamedAction::NextPage),
);
```

### `HyperlinkManager`

```rust
use rust_widgets::pdf::hyperlink::HyperlinkManager;

let mut manager = HyperlinkManager::new();

// Add links
manager.add_link(web_link);
manager.add_link(page_link);
println!("Total links: {}", manager.link_count());

// Named destinations
let dest = NamedDestination::new("chapter-1".into(), 1, 0.0, 0.0)
    .with_zoom(1.5);
manager.add_named_destination(dest);

println!("Destinations: {}", manager.destination_count());

// Query by page
let page_links = manager.get_page_links(0);
for link in &page_links {
    println!("Link on page {}: {:?}", link.page, link.action);
}

// Hit testing
if let Some(link) = manager.get_link_at_point(0, 150, 510) {
    println!("Clicked: {}", link.tooltip);
}

// Get named destination
if let Some(dest) = manager.get_named_destination("chapter-1") {
    println!("Navigate to page {} at zoom {}", dest.page, dest.zoom);
}

// Cleanup
manager.clear();
```

### `LinkBorder` & `HighlightMode`

```rust
use rust_widgets::pdf::hyperlink::{LinkBorder, HighlightMode};

let border = LinkBorder {
    horizontal_corner_radius: 2.0,
    vertical_corner_radius: 2.0,
    border_width: 1.0,
    dash_pattern: Some(vec![3.0, 2.0]),  // dashed border
};

let link = Hyperlink::new(/* ... */)
    .with_border(border)
    .with_highlight_mode(HighlightMode::Invert);  // or None, Outline, Push
```

---

## 7. Metadata & Security

### `PdfMetadata`

```rust
use rust_widgets::pdf::metadata::PdfMetadata;

let metadata = PdfMetadata {
    title: "Quarterly Report".into(),
    author: "Jane Smith".into(),
    subject: "Q2 2026 Financial Results".into(),
    keywords: "finance, quarterly, 2026, report".into(),
    creator: "rust-widgets 0.9.6".into(),
    producer: "rust-widgets PDF Engine".into(),
    creation_date: "2026-06-10T12:00:00Z".into(),
    modification_date: "2026-06-10T12:00:00Z".into(),
};

doc.set_metadata(metadata);
let meta = doc.metadata();
println!("Title: {}", meta.title);
```

### `PdfSecurity` (Diagnostic)

Security options are provided for diagnostic/documentation purposes:

```rust
use rust_widgets::pdf::security::PdfSecurity;

let security = PdfSecurity {
    owner_password: Some("owner123".into()),
    user_password: Some("user123".into()),
    allow_print: true,
    allow_modify: true,
    allow_copy: true,
    allow_annotate: true,
    allow_fill_forms: true,
    allow_accessibility: true,
    allow_assemble: true,
    allow_print_high_quality: true,
};

doc.set_security(security);
let sec = doc.security();
println!("Print allowed: {}", sec.allow_print);
```

---

## 8. SVG-Based Export Pipeline (`PdfExporter`)

`PdfExporter` converts widget trees to PDF via SVG intermediate rendering,
ensuring pixel-accurate output.

### Page Sizes

```rust
use rust_widgets::pdf::export::PageSize;

// Standard sizes
let a4 = PageSize::A4.dimensions();        // 595.0 × 842.0 pt
let letter = PageSize::Letter.dimensions(); // 612.0 × 792.0 pt

// Custom size
let custom = PageSize::Custom { width: 400.0, height: 600.0 };

// Convert to Size
let size = PageSize::A4.to_size();
println!("A4: {}×{} pt", size.width, size.height);
```

### Orientation

```rust
use rust_widgets::pdf::export::PdfOrientation;

let orientation = PdfOrientation::Landscape;
let dims = PageSize::A4.dimensions();
let (width, height) = orientation.apply(dims);
// Landscape: width = 842.0, height = 595.0
```

### Export Settings

```rust
use rust_widgets::pdf::export::{PdfExportSettings, PageSize, PdfOrientation};

let settings = PdfExportSettings {
    page_size: PageSize::A4,
    orientation: PdfOrientation::Portrait,
    margins: (20.0, 20.0, 20.0, 20.0),  // top, right, bottom, left
    dpi: 96.0,
};

println!("Effective: {}×{} pt", settings.effective_dimensions().0, settings.effective_dimensions().1);
println!("Content:   {}×{} pt", settings.content_width(), settings.content_height());
println!("Pixel:     {}×{} px", settings.pixel_size().0, settings.pixel_size().1);
```

### Exporting Widgets

```rust
use rust_widgets::pdf::export::{PdfExporter, PdfExportSettings, PageSize};

// Create exporter with custom settings
let exporter = PdfExporter::with_settings(PdfExportSettings {
    page_size: PageSize::A4,
    ..Default::default()
});

// Export a widget tree to PDF
let widgets: Vec<&dyn Widget> = vec![&root_widget];
exporter.export(&widgets, "report.pdf").expect("Export failed");
```

### Rendering Individual Pages

```rust
// Render pages from widgets
let pages = exporter.render_pages(&widgets);
for page in &pages {
    println!("Page {}: {}×{} pt ({}×{} px)",
        page.index, page.width_pt, page.height_pt,
        page.width_px, page.height_px);
    // page.svg_content contains the SVG for this page
}
```

### One-Shot Export

```rust
use rust_widgets::pdf::export::export_to_pdf;

export_to_pdf(&widgets, "output.pdf").expect("Export failed");
```

---

## 9. Print System

### `PrintDocument` Trait

```rust
pub trait PrintDocument {
    fn page_count(&self) -> u32;
    fn draw_page(&self, page_num: u32, context: &mut dyn PrintContext);
}
```

### `PrintContext` Trait

```rust
pub trait PrintContext {
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32);
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32);
    fn draw_rect(&mut self, rect: Rect, width: f32);
    fn fill_rect(&mut self, rect: Rect, color: u32);
    fn draw_image(&mut self, image: &[u8], rect: Rect);
    fn page_size(&self) -> Size;
}
```

### Implementing `PrintDocument`

```rust
use rust_widgets::print::{PrintDocument, PrintContext};
use rust_widgets::core::{Rect, Size};

struct InvoiceDocument {
    items: Vec<InvoiceItem>,
}

impl PrintDocument for InvoiceDocument {
    fn page_count(&self) -> u32 {
        // 1 page per 20 items
        ((self.items.len() as u32).saturating_sub(1) / 20 + 1).max(1)
    }

    fn draw_page(&self, page_num: u32, context: &mut dyn PrintContext) {
        let size = context.page_size();

        // Page header
        context.draw_text("INVOICE", 50.0, 50.0, 18.0);
        context.draw_line(50.0, 60.0, size.width as f32 - 50.0, 60.0, 1.0);

        // Table header
        context.fill_rect(Rect::new(50, 70, (size.width as i32 - 100).max(0) as u32, 20), 0xCCCCCC);
        context.draw_text("Item", 60.0, 82.0, 11.0);
        context.draw_text("Qty", 300.0, 82.0, 11.0);
        context.draw_text("Price", 400.0, 82.0, 11.0);

        // Item rows
        let start = (page_num * 20) as usize;
        let end = ((start + 20).min(self.items.len())) as usize;
        for (i, item) in self.items[start..end].iter().enumerate() {
            let y = 100.0 + (i as f32) * 20.0;
            context.draw_text(&item.name, 60.0, y, 10.0);
            context.draw_text(&item.qty.to_string(), 300.0, y, 10.0);
            context.draw_text(&format!("${:.2}", item.price), 400.0, y, 10.0);
        }

        // Footer
        let bottom = size.height as f32 - 30.0;
        context.draw_text(&format!("Page {} / {}", page_num + 1, self.page_count()),
            50.0, bottom, 9.0);
    }
}
```

---

## 10. `PrintPagination` — Flexible Page Range DSL

```rust
use rust_widgets::print::{PrintPagination, PageOrder, PageFilter};

let mut pagination = PrintPagination::new();

// Set specific page ranges via DSL
pagination.set_ranges_from_spec("1-3,5,8-10").unwrap();
// Prints pages 1,2,3,5,8,9,10

// Or set a single range
pagination.set_range(1, 5);  // Pages 1-5

// Add additional ranges
pagination.add_range(7, 9);   // Pages 7-9 in addition

// Multi-copy with collation
pagination.set_copies(3);
pagination.set_collate(true);       // AABBCC vs AAABBBCCC

// Page ordering
pagination.set_page_order(PageOrder::Descending);

// Parity filtering
pagination.set_page_filter(PageFilter::Odd);  // Only odd pages

// Clear all explicit ranges (back to "all pages")
pagination.clear_ranges();
```

### Page Range DSL Examples

| Spec | Pages Included |
|------|----------------|
| `""` | All pages |
| `"1-5"` | 1, 2, 3, 4, 5 |
| `"1,3,5"` | 1, 3, 5 |
| `"1-3,7,9-10"` | 1, 2, 3, 7, 9, 10 |
| `"5-1"` | 1, 2, 3, 4, 5 (auto-sorted) |

---

## 11. `Printer` — Platform Backend Selection

`Printer` selects the appropriate print backend based on the platform:

```rust
use rust_widgets::print::Printer;

let printer = Printer::new();
// Auto-detects: lp/lpr on Unix, print on Windows

// Print with default pagination (all pages, one copy)
printer.print(&my_document);

// Print with custom pagination
let mut pagination = PrintPagination::new();
pagination.set_range(1, 5);
pagination.set_copies(2);
printer.print_with_pagination(&my_document, &pagination);

// Print with result checking
match printer.print_with_result(&my_document) {
    Ok(()) => println!("Print job submitted successfully"),
    Err(e) => eprintln!("Print failed: {}", e),
}

// Print with pagination and result checking
match printer.print_with_pagination_result(&my_document, &pagination) {
    Ok(()) => println!("Paginated print submitted"),
    Err(e) => eprintln!("Paginated print failed: {}", e),
}
```

### Print Backend Detection

```rust
// Unix: checks lp --version or lpr --version
// Windows: checks print /?
```

---

## 12. `PrintDialog`

```rust
use rust_widgets::print::{PrintDialog, PrintPagination, PageOrder, PageFilter};

let mut dialog = PrintDialog::new();

// Configure via dialog
dialog.set_copies(2);

// Access pagination settings
dialog.pagination_mut().set_range(1, 10);
dialog.pagination_mut().set_page_order(PageOrder::Ascending);
dialog.pagination_mut().set_collate(true);
dialog.pagination_mut().set_page_filter(PageFilter::All);

// Show the dialog (checks for native print spooler)
if dialog.show() {
    println!("Dialog accepted");
} else {
    eprintln!("No print spooler available");
}

// Check if dialog was successfully shown
if dialog.was_shown() {
    println!("User confirmed print");
}
```

---

## 13. `PrintPreviewDialog`

```rust
use rust_widgets::print::PrintPreviewDialog;

let doc = Box::new(MyPrintDocument::new());
let mut preview = PrintPreviewDialog::new(doc);

println!("Total pages: {}", preview.page_count());

// Navigate pages
preview.next_page();
println!("Current page: {}", preview.current_page());

preview.prev_page();
println!("Current page: {}", preview.current_page());

// Show preview (renders document with Memory backend)
if preview.show() {
    let commands = preview.preview_commands();
    println!("Preview generated: {} commands", commands.len());
}
```

---

## 14. Print Manager with Job Lifecycle

```rust
use rust_widgets::print::{Printer, PrintDialog, PrintPreviewDialog, PrintPagination};

struct PrintManager {
    printer: Printer,
    dialog: PrintDialog,
}

impl PrintManager {
    fn new() -> Self {
        Self {
            printer: Printer::new(),
            dialog: PrintDialog::new(),
        }
    }

    fn print_document(&mut self, doc: &dyn PrintDocument, range: &str) -> Result<(), String> {
        let mut pagination = PrintPagination::new();
        pagination.set_ranges_from_spec(range)?;

        if self.dialog.show() {
            // Merge dialog pagination
            let dialog_pagination = self.dialog.pagination().clone();
            self.printer.print_with_pagination_result(doc, &dialog_pagination)
        } else {
            // Use programmatic pagination
            self.printer.print_with_pagination_result(doc, &pagination)
        }
    }

    fn preview_document(&self, doc: Box<dyn PrintDocument>) {
        let mut preview = PrintPreviewDialog::new(doc);
        if preview.show() {
            println!("Preview ready: {} pages", preview.page_count());
        }
    }
}
```

---

## 15. Complete Invoice PDF Example

```rust
use rust_widgets::pdf::{PdfWriter, PdfDocument, PdfPage};
use rust_widgets::core::{Color, Rect, Size};

fn generate_invoice() -> Result<(), std::io::Error> {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 595.0, height: 842.0 });

    if let Some(page) = doc.get_page(0) {
        // Company header
        page.draw_text("ACME Corporation", 50.0, 780.0, 22.0,
            Color { r: 33, g: 33, b: 33, a: 255 });
        page.draw_text("123 Business Ave, Suite 100", 50.0, 760.0, 10.0,
            Color { r: 100, g: 100, b: 100, a: 255 });
        page.draw_text("invoice@acmecorp.com", 50.0, 748.0, 10.0,
            Color { r: 100, g: 100, b: 100, a: 255 });

        // Invoice title
        page.draw_text("INVOICE", 400.0, 780.0, 28.0,
            Color { r: 66, g: 133, b: 244, a: 255 });
        page.draw_text("# INV-2026-0042", 400.0, 758.0, 12.0,
            Color { r: 80, g: 80, b: 80, a: 255 });

        // Separator
        page.draw_line(50.0, 730.0, 545.0, 730.0, 2.0,
            Color { r: 66, g: 133, b: 244, a: 255 });

        // Bill to / Date
        page.draw_text("Bill To:", 50.0, 710.0, 11.0,
            Color { r: 100, g: 100, b: 100, a: 255 });
        page.draw_text("Jane Doe", 50.0, 696.0, 12.0,
            Color { r: 33, g: 33, b: 33, a: 255 });
        page.draw_text("456 Client Road", 50.0, 684.0, 11.0,
            Color { r: 80, g: 80, b: 80, a: 255 });

        page.draw_text("Date: June 10, 2026", 400.0, 710.0, 11.0,
            Color { r: 80, g: 80, b: 80, a: 255 });
        page.draw_text("Due:  July 10, 2026", 400.0, 696.0, 11.0,
            Color { r: 80, g: 80, b: 80, a: 255 });

        // Table header
        let header_bg = Color { r: 66, g: 133, b: 244, a: 255 };
        let header_text = Color { r: 255, g: 255, b: 255, a: 255 };
        page.fill_rect(Rect::new(50, 640, 495, 24), header_bg);
        page.draw_text("Description", 60.0, 648.0, 11.0, header_text);
        page.draw_text("Qty", 320.0, 648.0, 11.0, header_text);
        page.draw_text("Rate", 400.0, 648.0, 11.0, header_text);
        page.draw_text("Amount", 480.0, 648.0, 11.0, header_text);

        // Line items
        let items = vec![
            ("Web Development", 40.0, 150.00),
            ("UI/UX Design", 20.0, 120.00),
            ("Server Setup", 5.0, 200.00),
        ];

        let mut y = 616.0;
        for (desc, hours, rate) in &items {
            page.draw_text(desc, 60.0, y, 10.0,
                Color { r: 40, g: 40, b: 40, a: 255 });
            page.draw_text(&format!("{:.0}", hours), 320.0, y, 10.0,
                Color { r: 40, g: 40, b: 40, a: 255 });
            page.draw_text(&format!("${:.2}", rate), 400.0, y, 10.0,
                Color { r: 40, g: 40, b: 40, a: 255 });
            page.draw_text(&format!("${:.2}", hours * rate), 480.0, y, 10.0,
                Color { r: 40, g: 40, b: 40, a: 255 });
            y -= 18.0;
        }

        // Total
        page.draw_line(350.0, y, 545.0, y, 1.0,
            Color { r: 150, g: 150, b: 150, a: 255 });
        let total: f64 = items.iter().map(|(_, h, r)| h * r).sum();
        page.draw_text(&format!("Total: ${:.2}", total), 400.0, y - 18.0, 14.0,
            Color { r: 33, g: 33, b: 33, a: 255 });

        // Footer
        page.draw_line(50.0, 80.0, 545.0, 80.0, 1.0,
            Color { r: 200, g: 200, b: 200, a: 255 });
        page.draw_text("Thank you for your business!", 50.0, 65.0, 9.0,
            Color { r: 120, g: 120, b: 120, a: 255 });
    }

    // Page numbering
    doc.set_page_numbering_enabled(true);
    doc.set_page_numbering_format("Page", 1);
    doc.set_page_numbering_layout(20.0, 20.0, 8.0);

    // Metadata
    doc.set_metadata(PdfMetadata {
        title: "Invoice INV-2026-0042".into(),
        author: "ACME Corporation".into(),
        ..Default::default()
    });

    doc.save("invoice.pdf")
}

fn main() {
    generate_invoice().expect("Failed to generate invoice PDF");
    println!("Invoice saved to invoice.pdf");
}
```

---

## 16. Architecture Summary

```
┌──────────────────────────────────────────────────┐
│                  PdfWriter                        │
│  create_document() / create_document_with_font()  │
└──────────────────────┬───────────────────────────┘
                       │
┌──────────────────────▼───────────────────────────┐
│              PdfDocument Trait                    │
│  add_page / get_page / save / to_bytes           │
│  metadata / security / page_numbering             │
└──────────────────────┬───────────────────────────┘
                       │
┌──────────────────────▼───────────────────────────┐
│               PdfPage Trait                       │
│  draw_text / draw_line / draw_rect / fill_rect    │
│  draw_image / add_text_field / add_checkbox       │
│  content() / form_fields()                        │
└──────────────────────┬───────────────────────────┘
                       │
     ┌─────────────────┼─────────────────┐
     ▼                 ▼                  ▼
┌──────────┐  ┌──────────────┐  ┌────────────────┐
│Annotation│  │  FormField   │  │  Hyperlink     │
│Manager   │  │  8 types     │  │  Manager       │
│29 types  │  │  serialized  │  │  URI/page/named│
└──────────┘  └──────────────┘  └────────────────┘
```

### Print Pipeline

```
┌──────────────────────┐
│    PrintDocument     │
│  page_count()        │
│  draw_page()         │
└──────────┬───────────┘
           │
┌──────────▼───────────┐     ┌──────────────────┐
│     PrintContext     │     │  PrintPagination  │
│  draw_text/line/rect │     │  ranges / copies  │
│  fill_rect/image     │     │  order / filter   │
└──────────┬───────────┘     └──────────────────┘
           │
┌──────────▼───────────┐
│       Printer        │
│  lp/lpr (Unix)       │
│  print (Windows)     │
└──────────┬───────────┘
           │
┌──────────▼───────────┐
│    PrintDialog       │
│  show() / was_shown()│
└──────────────────────┘
```

| Component | Role |
|-----------|------|
| `PdfWriter` | Factory: creates documents, detects backend |
| `PdfDocument` trait | Multi-page document management |
| `PdfPage` trait | Per-page drawing operators and form fields |
| `Annotation` / `AnnotationManager` | 29 annotation types with flags |
| `FormField` / `PdfFormField` | 8 field types with full serialization |
| `Hyperlink` / `HyperlinkManager` | URI, page, and named-action links |
| `PdfMetadata` | Document title, author, dates |
| `PdfSecurity` | Diagnostic security model |
| `PdfExporter` | Widgets → SVG → PDF export pipeline |
| `PageSize` / `PdfOrientation` | Standard page sizes and orientation |
| `PdfExportSettings` | DPI, margins, page size configuration |
| `PrintDocument` trait | Printable document interface |
| `PrintContext` trait | Drawing context for print rendering |
| `PrintPagination` | Page range DSL, copies, order, filter |
| `Printer` | Platform-specific print backend selection |
| `PrintDialog` | Native print dialog integration |
| `PrintPreviewDialog` | Document preview with Memory backend |
