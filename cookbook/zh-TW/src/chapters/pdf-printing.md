# PDF 與列印 (PDF & Printing)

rust-widgets 提供 PDF 文件生成，包含繪圖運算子、註解、表單欄位、超連結、中繼資料，以及基於 SVG 的匯出管線——外加一套包含分頁、平台後端選擇和列印對話框的列印系統。

---

## 1. PDF 生成概述

PDF 模組提供：

- **`PdfDocument` 特徵**：建立、修改和儲存多頁 PDF
- **`PdfPage` 特徵**：在頁面上繪製文字、線條、矩形和圖片
- **`PdfWriter`**：用於建立文件的便利工廠
- **註解**：29 種註解型別，支援自訂屬性
- **表單欄位**：8 種欄位型別，支援完整序列化
- **超連結**：URI、頁面導覽和具名動作
- **中繼資料與安全性**：文件屬性與診斷安全模型
- **SVG 匯出管線**：用於 widget 轉 PDF 渲染的 `PdfExporter`

---

## 2. `PdfDocument` 特徵

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

### 使用 `PdfWriter` 建立文件

```rust
use rust_widgets::pdf::PdfWriter;
use rust_widgets::core::Size;

// 建立寫入器 (自動檢測用於列印整合的平台後端)
let writer = PdfWriter::new();
println!("PDF backend: {}", writer.backend_name());

// 建立 A4 文件
let mut doc = writer.create_document(Size { width: 595.0, height: 842.0 });

// 或使用內嵌字型建立
let mut doc = writer.create_document_with_font_path(
    Size { width: 595.0, height: 842.0 },
    "CustomFont",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
).expect("Failed to load font");
```

### 頁面管理

```rust
// 新增頁面
let page1 = doc.add_page(Size { width: 595.0, height: 842.0 });
let page2 = doc.add_page(Size { width: 595.0, height: 842.0 });

// 在指定索引插入頁面
doc.insert_page(1, Size { width: 595.0, height: 842.0 });

// 移除頁面 (至少保留一頁)
doc.remove_page(0);

// 重新排序頁面
doc.reorder_pages(&[2, 0, 1]);

println!("Page count: {}", doc.page_count());
```

### 頁碼

```rust
doc.set_page_numbering_enabled(true);
doc.set_page_numbering_format("Page", 1);
doc.set_page_numbering_layout(20.0, 20.0, 10.0);
// 產生頁尾："Page 1"、"Page 2" 等
```

### 儲存

```rust
// 儲存為檔案
doc.save("output.pdf").expect("Failed to save PDF");

// 取得原始位元組 (用於網路傳輸、嵌入等)
let bytes = doc.to_bytes().expect("Failed to generate PDF bytes");
std::fs::write("output.pdf", &bytes).unwrap();
```

---

## 3. `PdfPage` 特徵 — 繪圖運算子

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

### 完整的繪圖範例

```rust
use rust_widgets::pdf::PdfWriter;
use rust_widgets::core::{Color, Rect, Size};

let writer = PdfWriter::new();
let mut doc = writer.create_document(Size { width: 595.0, height: 842.0 });

// 取得第一頁
if let Some(page) = doc.get_page(0) {
    // 在指定位置繪製文字 (PDF 座標系統 y=0 在底部)
    page.draw_text(
        "Invoice #12345",
        50.0, 750.0,       // x, y 以點為單位
        24.0,               // 字型大小
        Color { r: 33, g: 33, b: 33, a: 255 },
    );

    // 副標題
    page.draw_text(
        "Date: 2026-06-10",
        50.0, 720.0,
        12.0,
        Color { r: 100, g: 100, b: 100, a: 255 },
    );

    // 分隔線
    page.draw_line(
        50.0, 700.0,        // 起點
        545.0, 700.0,       // 終點
        1.0,                // 寬度
        Color { r: 200, g: 200, b: 200, a: 255 },
    );

    // 填滿的標題列
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

    // 邊框矩形
    page.draw_rect(
        Rect::new(50, 550, 495, 100),
        2.0,
        Color { r: 100, g: 100, b: 100, a: 255 },
    );

    // 繪製圖片 (PNG/JPEG 原始位元組)
    let image_bytes = std::fs::read("logo.png").unwrap_or_default();
    page.draw_image(&image_bytes, Rect::new(400, 720, 145, 60));
}

doc.save("invoice.pdf").expect("Failed to save");
```

### 座標系統

PDF 使用左下角原點。`to_pdf_y()` 函式將左上角座標轉換為 PDF 座標：

```rust
// y=0 在頂部 (widget 座標) → y=842 在底部 (PDF 座標)
let pdf_y = page_size.height - y;
```

---

## 4. 註解 (29 種型別)

### `AnnotationType` 列舉

| # | 型別 | 說明 |
|---|------|-------------|
| 1 | `Text` | 便利貼 / 文字註解 |
| 2 | `Highlight` | 文字螢光筆標記 |
| 3 | `Underline` | 文字底線標記 |
| 4 | `StrikeOut` | 刪除線標記 |
| 5 | `Squiggly` | 波浪底線 |
| 6 | `Link` | 超連結註解 |
| 7 | `Popup` | 標記用的彈出視窗 |
| 8 | `Line` | 線條註解 |
| 9 | `Square` | 矩形註解 |
| 10 | `Circle` | 圓形/橢圓註解 |
| 11 | `Polygon` | 封閉多邊形 |
| 12 | `PolyLine` | 開放折線 |
| 13 | `Ink` | 手繪「鉛筆」註解 |
| 14 | `Stamp` | 橡皮圖章 |
| 15 | `Caret` | 插入點標記 |
| 16 | `FileAttachment` | 嵌入檔案 |
| 17 | `Sound` | 音訊註解 |
| 18 | `Movie` | 影片註解 |
| 19 | `Widget` | 表單欄位 widget |
| 20 | `Screen` | 螢幕註解 |
| 21 | `PrinterMark` | 印刷標記 |
| 22 | `TrapNet` | 陷印網路 |
| 23 | `Watermark` | 浮水印 |
| 24 | `ThreeD` | 3D 註解 |
| 25–29 | *(保留)* | 未來擴充 |

### 建立註解

```rust
use rust_widgets::pdf::annotation::{Annotation, AnnotationType, AnnotationFlags};
use rust_widgets::core::{Color, Rect};

let annotation = Annotation::new(
    "ann-1".into(),     // id
    0,                   // 頁面索引
    AnnotationType::Highlight,
    Rect::new(50, 400, 200, 20),
)
.with_contents("Important: review this section".into())
.with_author("John Doe".into())
.with_color(Color { r: 255, g: 255, b: 0, a: 128 })
.with_opacity(0.5);

// 檢查可見性
if annotation.is_visible() {
    println!("Annotation is visible");
}
```

### `AnnotationFlags`

```rust
pub struct AnnotationFlags {
    pub hidden: bool,        // 不顯示
    pub invisible: bool,     // 未懸停時不顯示
    pub locked: bool,        // 無法刪除
    pub locked_contents: bool, // 內容無法修改
    pub print: bool,         // 列印註解
    pub no_zoom: bool,       // 不隨縮放縮放
    pub no_rotate: bool,     // 不隨頁面旋轉
    pub no_view: bool,       // 不顯示在螢幕上
    pub read_only: bool,     // 無法與之互動
    pub toggle_no_view: bool, // 切換可見性行為
}
```

---

## 5. 表單欄位 (8 種型別)

### `FieldType` 列舉

| # | 型別 | 說明 |
|---|------|-------------|
| 1 | `Text` | 單行或多行文字輸入 |
| 2 | `Checkbox` | 二元核取方塊 |
| 3 | `Radio` | 選項按鈕群組 |
| 4 | `ListBox` | 可滾動清單選擇 |
| 5 | `ComboBox` | 下拉式選擇 |
| 6 | `Button` | 按鈕 |
| 7 | `Signature` | 數位簽章欄位 |
| 8 | *(保留)* | |

### `FormField` 結構

```rust
use rust_widgets::pdf::form::{FormField, FieldType};
use rust_widgets::core::{Color, Rect};

let field = FormField::new(
    "field-name".into(),    // id
    "CustomerName".into(),  // name
    FieldType::Text,
    0,                      // 頁面
    Rect::new(100, 500, 300, 20),
);

// 使用建構器風格方法設定
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

### 表單欄位屬性

| 屬性 | 型別 | 說明 |
|----------|------|-------------|
| `id` | `String` | 唯一識別符 |
| `name` | `String` | 欄位名稱 (表單資料鍵) |
| `field_type` | `FieldType` | 8 種型別之一 |
| `page` | `u32` | 頁面索引 |
| `rect` | `Rect` | 邊界矩形 |
| `value` | `String` | 目前值 |
| `default_value` | `String` | 預設值 |
| `is_read_only` | `bool` | 防止編輯 |
| `is_required` | `bool` | 提交時必填 |
| `is_hidden` | `bool` | 隱藏欄位 |
| `tooltip` | `String` | 懸浮提示 |
| `font_name` | `String` | 字型家族 (預設："Helvetica") |
| `font_size` | `f32` | 字型大小 (預設：12.0) |
| `text_color` | `Color` | 文字顏色 |
| `background_color` | `Option<Color>` | 背景填滿 |
| `border_color` | `Option<Color>` | 邊框顏色 |
| `border_width` | `f32` | 邊框寬度 |
| `options` | `Vec<String>` | 清單/下拉式選項 |
| `max_length` | `Option<u32>` | 最大文字長度 |
| `is_multiline` | `bool` | 多行文字 |
| `is_password` | `bool` | 密碼遮罩 |
| `is_spell_check_enabled` | `bool` | 拼字檢查 |
| `is_scrollable` | `bool` | 可滾動文字區域 |

### 透過 PdfPage 新增表單欄位

```rust
if let Some(page) = doc.get_page(0) {
    // 文字欄位
    page.add_text_field("email", Rect::new(100, 700, 200, 20), "user@example.com");

    // 核取方塊
    page.add_checkbox("subscribe", Rect::new(100, 670, 15, 15), true);

    // 按鈕
    page.add_button("submit", Rect::new(100, 620, 120, 30), "Submit");

    // 取得所有表單欄位
    let fields = page.form_fields();
    for field in &fields {
        println!("Field: {}", field.name);
    }
}
```

### `PdfFormField` 列舉 (序列化)

```rust
pub enum PdfFormField {
    TextField { name: String, rect: Rect, value: String },
    CheckBox { name: String, rect: Rect, checked: bool },
    Button { name: String, rect: Rect, text: String },
    // ... 還有 radio、list、combo、signature 等其他變體
}
```

---

## 6. 超連結

### `LinkAction` 列舉

```rust
pub enum LinkAction {
    GoToPage { page: u32, x: f32, y: f32 },          // 導覽到頁面
    GoToNamedDestination(String),                      // 具名目的地
    Uri(String),                                       // 外部 URL
    LaunchFile(String),                                // 啟動外部檔案
    JavaScript(String),                                // 執行 JS 動作
    NamedAction(NamedAction),                          // 標準動作
}

pub enum NamedAction {
    NextPage, PrevPage, FirstPage, LastPage,
    Print, SaveAs,
}
```

### 建立超連結

```rust
use rust_widgets::pdf::hyperlink::{Hyperlink, LinkAction, HighlightMode, NamedDestination};
use rust_widgets::core::Rect;

// URI 超連結
let link = Hyperlink::new(
    "link-1".into(),
    0,
    Rect::new(100, 500, 200, 20),
    LinkAction::Uri("https://example.com".into()),
)
.with_tooltip("Visit our website".into());

// 頁面導覽
let link = Hyperlink::new(
    "link-2".into(),
    0,
    Rect::new(100, 450, 200, 20),
    LinkAction::GoToPage { page: 3, x: 0.0, y: 0.0 },
)
.with_tooltip("Go to page 3".into());

// 具名動作
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

// 新增連結
manager.add_link(web_link);
manager.add_link(page_link);
println!("Total links: {}", manager.link_count());

// 具名目的地
let dest = NamedDestination::new("chapter-1".into(), 1, 0.0, 0.0)
    .with_zoom(1.5);
manager.add_named_destination(dest);

println!("Destinations: {}", manager.destination_count());

// 依頁面查詢
let page_links = manager.get_page_links(0);
for link in &page_links {
    println!("Link on page {}: {:?}", link.page, link.action);
}

// 點擊測試
if let Some(link) = manager.get_link_at_point(0, 150, 510) {
    println!("Clicked: {}", link.tooltip);
}

// 取得具名目的地
if let Some(dest) = manager.get_named_destination("chapter-1") {
    println!("Navigate to page {} at zoom {}", dest.page, dest.zoom);
}

// 清理
manager.clear();
```

### `LinkBorder` 與 `HighlightMode`

```rust
use rust_widgets::pdf::hyperlink::{LinkBorder, HighlightMode};

let border = LinkBorder {
    horizontal_corner_radius: 2.0,
    vertical_corner_radius: 2.0,
    border_width: 1.0,
    dash_pattern: Some(vec![3.0, 2.0]),  // 虛線邊框
};

let link = Hyperlink::new(/* ... */)
    .with_border(border)
    .with_highlight_mode(HighlightMode::Invert);  // 或 None, Outline, Push
```

---

## 7. 中繼資料與安全性

### `PdfMetadata`

```rust
use rust_widgets::pdf::metadata::PdfMetadata;

let metadata = PdfMetadata {
    title: "Quarterly Report".into(),
    author: "Jane Smith".into(),
    subject: "Q2 2026 Financial Results".into(),
    keywords: "finance, quarterly, 2026, report".into(),
    creator: "rust-widgets 1.0.0".into(),
    producer: "rust-widgets PDF Engine".into(),
    creation_date: "2026-06-10T12:00:00Z".into(),
    modification_date: "2026-06-10T12:00:00Z".into(),
};

doc.set_metadata(metadata);
let meta = doc.metadata();
println!("Title: {}", meta.title);
```

### `PdfSecurity` (診斷)

提供安全選項用於診斷/文件目的：

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

## 8. 基於 SVG 的匯出管線 (`PdfExporter`)

`PdfExporter` 透過 SVG 中間渲染將 widget 樹轉換為 PDF，確保像素精確的輸出。

### 頁面尺寸

```rust
use rust_widgets::pdf::export::PageSize;

// 標準尺寸
let a4 = PageSize::A4.dimensions();        // 595.0 × 842.0 pt
let letter = PageSize::Letter.dimensions(); // 612.0 × 792.0 pt

// 自訂尺寸
let custom = PageSize::Custom { width: 400.0, height: 600.0 };

// 轉換為 Size
let size = PageSize::A4.to_size();
println!("A4: {}×{} pt", size.width, size.height);
```

### 方向

```rust
use rust_widgets::pdf::export::PdfOrientation;

let orientation = PdfOrientation::Landscape;
let dims = PageSize::A4.dimensions();
let (width, height) = orientation.apply(dims);
// 橫向：width = 842.0, height = 595.0
```

### 匯出設定

```rust
use rust_widgets::pdf::export::{PdfExportSettings, PageSize, PdfOrientation};

let settings = PdfExportSettings {
    page_size: PageSize::A4,
    orientation: PdfOrientation::Portrait,
    margins: (20.0, 20.0, 20.0, 20.0),  // 上、右、下、左
    dpi: 96.0,
};

println!("Effective: {}×{} pt", settings.effective_dimensions().0, settings.effective_dimensions().1);
println!("Content:   {}×{} pt", settings.content_width(), settings.content_height());
println!("Pixel:     {}×{} px", settings.pixel_size().0, settings.pixel_size().1);
```

### 匯出 Widget

```rust
use rust_widgets::pdf::export::{PdfExporter, PdfExportSettings, PageSize};

// 使用自訂設定建立匯出器
let exporter = PdfExporter::with_settings(PdfExportSettings {
    page_size: PageSize::A4,
    ..Default::default()
});

// 將 widget 樹匯出為 PDF
let widgets: Vec<&dyn Widget> = vec![&root_widget];
exporter.export(&widgets, "report.pdf").expect("Export failed");
```

### 渲染個別頁面

```rust
// 從 widgets 渲染頁面
let pages = exporter.render_pages(&widgets);
for page in &pages {
    println!("Page {}: {}×{} pt ({}×{} px)",
        page.index, page.width_pt, page.height_pt,
        page.width_px, page.height_px);
    // page.svg_content 包含此頁面的 SVG
}
```

### 一次性匯出

```rust
use rust_widgets::pdf::export::export_to_pdf;

export_to_pdf(&widgets, "output.pdf").expect("Export failed");
```

---

## 9. 列印系統

### `PrintDocument` 特徵

```rust
pub trait PrintDocument {
    fn page_count(&self) -> u32;
    fn draw_page(&self, page_num: u32, context: &mut dyn PrintContext);
}
```

### `PrintContext` 特徵

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

### 實作 `PrintDocument`

```rust
use rust_widgets::print::{PrintDocument, PrintContext};
use rust_widgets::core::{Rect, Size};

struct InvoiceDocument {
    items: Vec<InvoiceItem>,
}

impl PrintDocument for InvoiceDocument {
    fn page_count(&self) -> u32 {
        // 每 20 個項目一頁
        ((self.items.len() as u32).saturating_sub(1) / 20 + 1).max(1)
    }

    fn draw_page(&self, page_num: u32, context: &mut dyn PrintContext) {
        let size = context.page_size();

        // 頁面標頭
        context.draw_text("INVOICE", 50.0, 50.0, 18.0);
        context.draw_line(50.0, 60.0, size.width as f32 - 50.0, 60.0, 1.0);

        // 表格標頭
        context.fill_rect(Rect::new(50, 70, (size.width as i32 - 100).max(0) as u32, 20), 0xCCCCCC);
        context.draw_text("Item", 60.0, 82.0, 11.0);
        context.draw_text("Qty", 300.0, 82.0, 11.0);
        context.draw_text("Price", 400.0, 82.0, 11.0);

        // 項目列
        let start = (page_num * 20) as usize;
        let end = ((start + 20).min(self.items.len())) as usize;
        for (i, item) in self.items[start..end].iter().enumerate() {
            let y = 100.0 + (i as f32) * 20.0;
            context.draw_text(&item.name, 60.0, y, 10.0);
            context.draw_text(&item.qty.to_string(), 300.0, y, 10.0);
            context.draw_text(&format!("${:.2}", item.price), 400.0, y, 10.0);
        }

        // 頁尾
        let bottom = size.height as f32 - 30.0;
        context.draw_text(&format!("Page {} / {}", page_num + 1, self.page_count()),
            50.0, bottom, 9.0);
    }
}
```

---

## 10. `PrintPagination` — 彈性頁面範圍 DSL

```rust
use rust_widgets::print::{PrintPagination, PageOrder, PageFilter};

let mut pagination = PrintPagination::new();

// 透過 DSL 設定特定頁面範圍
pagination.set_ranges_from_spec("1-3,5,8-10").unwrap();
// 列印頁面 1,2,3,5,8,9,10

// 或設定單一範圍
pagination.set_range(1, 5);  // 頁面 1-5

// 新增額外範圍
pagination.add_range(7, 9);   // 外加頁面 7-9

// 多份列印含排序
pagination.set_copies(3);
pagination.set_collate(true);       // AABBCC vs AAABBBCCC

// 頁面排序
pagination.set_page_order(PageOrder::Descending);

// 奇偶篩選
pagination.set_page_filter(PageFilter::Odd);  // 僅奇數頁

// 清除所有明確範圍 (回到「所有頁面」)
pagination.clear_ranges();
```

### 頁面範圍 DSL 範例

| 規格 | 包含的頁面 |
|------|----------------|
| `""` | 所有頁面 |
| `"1-5"` | 1, 2, 3, 4, 5 |
| `"1,3,5"` | 1, 3, 5 |
| `"1-3,7,9-10"` | 1, 2, 3, 7, 9, 10 |
| `"5-1"` | 1, 2, 3, 4, 5 (自動排序) |

---

## 11. `Printer` — 平台後端選擇

`Printer` 根據平台選擇適當的列印後端：

```rust
use rust_widgets::print::Printer;

let printer = Printer::new();
// 自動檢測：Unix 上的 lp/lpr，Windows 上的 print

// 使用預設分頁列印 (所有頁面，一份)
printer.print(&my_document);

// 使用自訂分頁列印
let mut pagination = PrintPagination::new();
pagination.set_range(1, 5);
pagination.set_copies(2);
printer.print_with_pagination(&my_document, &pagination);

// 列印並檢查結果
match printer.print_with_result(&my_document) {
    Ok(()) => println!("Print job submitted successfully"),
    Err(e) => eprintln!("Print failed: {}", e),
}

// 含分頁與結果檢查
match printer.print_with_pagination_result(&my_document, &pagination) {
    Ok(()) => println!("Paginated print submitted"),
    Err(e) => eprintln!("Paginated print failed: {}", e),
}
```

### 列印後端檢測

```rust
// Unix：檢查 lp --version 或 lpr --version
// Windows：檢查 print /?
```

---

## 12. `PrintDialog`

```rust
use rust_widgets::print::{PrintDialog, PrintPagination, PageOrder, PageFilter};

let mut dialog = PrintDialog::new();

// 透過對話框設定
dialog.set_copies(2);

// 存取分頁設定
dialog.pagination_mut().set_range(1, 10);
dialog.pagination_mut().set_page_order(PageOrder::Ascending);
dialog.pagination_mut().set_collate(true);
dialog.pagination_mut().set_page_filter(PageFilter::All);

// 顯示對話框 (檢查原生列印多工緩衝處理器)
if dialog.show() {
    println!("Dialog accepted");
} else {
    eprintln!("No print spooler available");
}

// 檢查對話框是否成功顯示
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

// 導覽頁面
preview.next_page();
println!("Current page: {}", preview.current_page());

preview.prev_page();
println!("Current page: {}", preview.current_page());

// 顯示預覽 (使用 Memory 後端渲染文件)
if preview.show() {
    let commands = preview.preview_commands();
    println!("Preview generated: {} commands", commands.len());
}
```

---

## 14. 含任務生命週期的列印管理器

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
            // 合併對話框分頁
            let dialog_pagination = self.dialog.pagination().clone();
            self.printer.print_with_pagination_result(doc, &dialog_pagination)
        } else {
            // 使用程式化分頁
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

## 15. 完整的發票 PDF 範例

```rust
use rust_widgets::pdf::{PdfWriter, PdfDocument, PdfPage};
use rust_widgets::core::{Color, Rect, Size};

fn generate_invoice() -> Result<(), std::io::Error> {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 595.0, height: 842.0 });

    if let Some(page) = doc.get_page(0) {
        // 公司標頭
        page.draw_text("ACME Corporation", 50.0, 780.0, 22.0,
            Color { r: 33, g: 33, b: 33, a: 255 });
        page.draw_text("123 Business Ave, Suite 100", 50.0, 760.0, 10.0,
            Color { r: 100, g: 100, b: 100, a: 255 });
        page.draw_text("invoice@acmecorp.com", 50.0, 748.0, 10.0,
            Color { r: 100, g: 100, b: 100, a: 255 });

        // 發票標題
        page.draw_text("INVOICE", 400.0, 780.0, 28.0,
            Color { r: 66, g: 133, b: 244, a: 255 });
        page.draw_text("# INV-2026-0042", 400.0, 758.0, 12.0,
            Color { r: 80, g: 80, b: 80, a: 255 });

        // 分隔線
        page.draw_line(50.0, 730.0, 545.0, 730.0, 2.0,
            Color { r: 66, g: 133, b: 244, a: 255 });

        // 帳單對象 / 日期
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

        // 表格標頭
        let header_bg = Color { r: 66, g: 133, b: 244, a: 255 };
        let header_text = Color { r: 255, g: 255, b: 255, a: 255 };
        page.fill_rect(Rect::new(50, 640, 495, 24), header_bg);
        page.draw_text("Description", 60.0, 648.0, 11.0, header_text);
        page.draw_text("Qty", 320.0, 648.0, 11.0, header_text);
        page.draw_text("Rate", 400.0, 648.0, 11.0, header_text);
        page.draw_text("Amount", 480.0, 648.0, 11.0, header_text);

        // 行項目
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

        // 總計
        page.draw_line(350.0, y, 545.0, y, 1.0,
            Color { r: 150, g: 150, b: 150, a: 255 });
        let total: f64 = items.iter().map(|(_, h, r)| h * r).sum();
        page.draw_text(&format!("Total: ${:.2}", total), 400.0, y - 18.0, 14.0,
            Color { r: 33, g: 33, b: 33, a: 255 });

        // 頁尾
        page.draw_line(50.0, 80.0, 545.0, 80.0, 1.0,
            Color { r: 200, g: 200, b: 200, a: 255 });
        page.draw_text("Thank you for your business!", 50.0, 65.0, 9.0,
            Color { r: 120, g: 120, b: 120, a: 255 });
    }

    // 頁碼
    doc.set_page_numbering_enabled(true);
    doc.set_page_numbering_format("Page", 1);
    doc.set_page_numbering_layout(20.0, 20.0, 8.0);

    // 中繼資料
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

## 16. 架構摘要

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

### 列印管線

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

| 元件 | 角色 |
|-----------|------|
| `PdfWriter` | 工廠：建立文件，檢測後端 |
| `PdfDocument` 特徵 | 多頁文件管理 |
| `PdfPage` 特徵 | 每頁繪圖運算子與表單欄位 |
| `Annotation` / `AnnotationManager` | 29 種註解型別，含旗標 |
| `FormField` / `PdfFormField` | 8 種欄位型別，含完整序列化 |
| `Hyperlink` / `HyperlinkManager` | URI、頁面與具名動作連結 |
| `PdfMetadata` | 文件標題、作者、日期 |
| `PdfSecurity` | 診斷安全模型 |
| `PdfExporter` | Widgets → SVG → PDF 匯出管線 |
| `PageSize` / `PdfOrientation` | 標準頁面尺寸與方向 |
| `PdfExportSettings` | DPI、邊距、頁面尺寸設定 |
| `PrintDocument` 特徵 | 可列印文件介面 |
| `PrintContext` 特徵 | 列印渲染的繪圖上下文 |
| `PrintPagination` | 頁面範圍 DSL、份數、順序、篩選 |
| `Printer` | 平台特定的列印後端選擇 |
| `PrintDialog` | 原生列印對話框整合 |
| `PrintPreviewDialog` | 使用 Memory 後端的文件預覽 |
