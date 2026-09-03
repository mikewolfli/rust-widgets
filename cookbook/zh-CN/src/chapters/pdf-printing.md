# PDF 与打印

rust-widgets 提供 PDF 文档生成功能，包括绘图操作符、注释、表单字段、超链接、元数据，以及基于 SVG 的导出管线——此外还提供带有分页、平台后端选择和打印对话框的打印系统。

---

## 1. PDF 生成概述

PDF 模块提供以下功能：

- **`PdfDocument` trait**：创建、修改和保存多页 PDF
- **`PdfPage` trait**：在页面上绘制文本、线条、矩形和图像
- **`PdfWriter`**：便捷工厂，用于创建文档
- **注释**：29 种注释类型，支持自定义属性
- **表单字段**：8 种字段类型，支持完整序列化
- **超链接**：URI、页面导航和命名操作
- **元数据与安全**：文档属性和诊断性安全模型
- **SVG 导出管线**：`PdfExporter`，用于将 widget 渲染为 PDF

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

### 使用 `PdfWriter` 创建文档

```rust
use rust_widgets::pdf::PdfWriter;
use rust_widgets::core::Size;

// 创建 writer（自动检测打印集成的平台后端）
let writer = PdfWriter::new();
println!("PDF 后端: {}", writer.backend_name());

// 创建 A4 文档
let mut doc = writer.create_document(Size { width: 595.0, height: 842.0 });

// 或者使用嵌入字体创建
let mut doc = writer.create_document_with_font_path(
    Size { width: 595.0, height: 842.0 },
    "CustomFont",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
).expect("加载字体失败");
```

### 页面管理

```rust
// 添加页面
let page1 = doc.add_page(Size { width: 595.0, height: 842.0 });
let page2 = doc.add_page(Size { width: 595.0, height: 842.0 });

// 在指定索引处插入页面
doc.insert_page(1, Size { width: 595.0, height: 842.0 });

// 删除页面（至少保留一页）
doc.remove_page(0);

// 重新排列页面顺序
doc.reorder_pages(&[2, 0, 1]);

println!("页面数: {}", doc.page_count());
```

### 页码

```rust
doc.set_page_numbering_enabled(true);
doc.set_page_numbering_format("Page", 1);
doc.set_page_numbering_layout(20.0, 20.0, 10.0);
// 生成页脚："Page 1", "Page 2" 等
```

### 保存

```rust
// 保存到文件
doc.save("output.pdf").expect("保存 PDF 失败");

// 获取原始字节（用于网络上传、嵌入等）
let bytes = doc.to_bytes().expect("生成 PDF 字节失败");
std::fs::write("output.pdf", &bytes).unwrap();
```

---

## 3. `PdfPage` Trait — 绘图操作符

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

### 完整绘图示例

```rust
use rust_widgets::pdf::PdfWriter;
use rust_widgets::core::{Color, Rect, Size};

let writer = PdfWriter::new();
let mut doc = writer.create_document(Size { width: 595.0, height: 842.0 });

// 获取第一页
if let Some(page) = doc.get_page(0) {
    // 在指定位置绘制文本（PDF 坐标中 y=0 在底部）
    page.draw_text(
        "Invoice #12345",
        50.0, 750.0,       // x, y（单位：点）
        24.0,               // 字体大小
        Color { r: 33, g: 33, b: 33, a: 255 },
    );

    // 副标题
    page.draw_text(
        "Date: 2026-06-10",
        50.0, 720.0,
        12.0,
        Color { r: 100, g: 100, b: 100, a: 255 },
    );

    // 分隔线
    page.draw_line(
        50.0, 700.0,        // 起点
        545.0, 700.0,       // 终点
        1.0,                // 线宽
        Color { r: 200, g: 200, b: 200, a: 255 },
    );

    // 填充表头条
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

    // 带边框矩形
    page.draw_rect(
        Rect::new(50, 550, 495, 100),
        2.0,
        Color { r: 100, g: 100, b: 100, a: 255 },
    );

    // 绘制图像（PNG/JPEG 原始字节）
    let image_bytes = std::fs::read("logo.png").unwrap_or_default();
    page.draw_image(&image_bytes, Rect::new(400, 720, 145, 60));
}

doc.save("invoice.pdf").expect("保存失败");
```

### 坐标系

PDF 使用左下角原点。`to_pdf_y()` 函数将左上角坐标转换为 PDF 坐标：

```rust
// y=0 在顶部（widget 坐标）→ y=842 在底部（PDF 坐标）
let pdf_y = page_size.height - y;
```

---

## 4. 注释（29 种类型）

### `AnnotationType` 枚举

| # | 类型 | 描述 |
|---|------|-------------|
| 1 | `Text` | 便签/文本注释 |
| 2 | `Highlight` | 文本高亮标记 |
| 3 | `Underline` | 文本下划线标记 |
| 4 | `StrikeOut` | 删除线标记 |
| 5 | `Squiggly` | 波浪线下划线 |
| 6 | `Link` | 超链接注释 |
| 7 | `Popup` | 标记的弹出窗口 |
| 8 | `Line` | 线条注释 |
| 9 | `Square` | 矩形注释 |
| 10 | `Circle` | 圆形/椭圆注释 |
| 11 | `Polygon` | 封闭多边形 |
| 12 | `PolyLine` | 开放折线 |
| 13 | `Ink` | 手写"画笔"注释 |
| 14 | `Stamp` | 橡皮图章 |
| 15 | `Caret` | 插入符（插入点） |
| 16 | `FileAttachment` | 嵌入文件 |
| 17 | `Sound` | 音频注释 |
| 18 | `Movie` | 影片注释 |
| 19 | `Widget` | 表单字段 widget |
| 20 | `Screen` | 屏幕注释 |
| 21 | `PrinterMark` | 印刷标记 |
| 22 | `TrapNet` | 陷印网络 |
| 23 | `Watermark` | 水印 |
| 24 | `ThreeD` | 3D 注释 |
| 25–29 | *(保留)* | 未来扩展 |

### 创建注释

```rust
use rust_widgets::pdf::annotation::{Annotation, AnnotationType, AnnotationFlags};
use rust_widgets::core::{Color, Rect};

let annotation = Annotation::new(
    "ann-1".into(),     // id
    0,                   // 页面索引
    AnnotationType::Highlight,
    Rect::new(50, 400, 200, 20),
)
.with_contents("重要：请审查此部分".into())
.with_author("张三".into())
.with_color(Color { r: 255, g: 255, b: 0, a: 128 })
.with_opacity(0.5);

// 检查可见性
if annotation.is_visible() {
    println!("注释可见");
}
```

### `AnnotationFlags`

```rust
pub struct AnnotationFlags {
    pub hidden: bool,        // 不显示
    pub invisible: bool,     // 未悬停时不显示
    pub locked: bool,        // 不可删除
    pub locked_contents: bool, // 内容不可修改
    pub print: bool,         // 打印注释
    pub no_zoom: bool,       // 不随缩放缩放
    pub no_rotate: bool,     // 不随页面旋转
    pub no_view: bool,       // 不在屏幕上显示
    pub read_only: bool,     // 不可交互
    pub toggle_no_view: bool, // 切换可见性行为
}
```

---

## 5. 表单字段（8 种类型）

### `FieldType` 枚举

| # | 类型 | 描述 |
|---|------|-------------|
| 1 | `Text` | 单行或多行文本输入 |
| 2 | `Checkbox` | 二进制复选框 |
| 3 | `Radio` | 单选按钮组 |
| 4 | `ListBox` | 可滚动列表选择 |
| 5 | `ComboBox` | 下拉选择 |
| 6 | `Button` | 按钮 |
| 7 | `Signature` | 数字签名字段 |
| 8 | *(保留)* | |

### `FormField` 结构体

```rust
use rust_widgets::pdf::form::{FormField, FieldType};
use rust_widgets::core::{Color, Rect};

let field = FormField::new(
    "field-name".into(),    // id
    "CustomerName".into(),  // 名称
    FieldType::Text,
    0,                      // 页面
    Rect::new(100, 500, 300, 20),
);

// 使用 builder 风格方法进行配置
field.with_value("张三".into())
    .with_default_value("输入姓名".into())
    .with_font_size(12.0)
    .with_text_color(Color { r: 0, g: 0, b: 0, a: 255 })
    .with_background_color(Color { r: 255, g: 255, b: 255, a: 255 })
    .with_border(Color { r: 150, g: 150, b: 150, a: 255 }, 1.0)
    .with_tooltip("请输入您的全名".into())
    .with_max_length(100)
    .with_multiline(false)
    .with_password(false)
    .with_read_only(false)
    .with_required(true);
```

### 表单字段属性

| 属性 | 类型 | 描述 |
|----------|------|-------------|
| `id` | `String` | 唯一标识符 |
| `name` | `String` | 字段名称（表单数据键） |
| `field_type` | `FieldType` | 8 种类型之一 |
| `page` | `u32` | 页面索引 |
| `rect` | `Rect` | 边界矩形 |
| `value` | `String` | 当前值 |
| `default_value` | `String` | 默认值 |
| `is_read_only` | `bool` | 禁止编辑 |
| `is_required` | `bool` | 提交时必须填写 |
| `is_hidden` | `bool` | 隐藏字段 |
| `tooltip` | `String` | 悬停提示 |
| `font_name` | `String` | 字体系列（默认："Helvetica"） |
| `font_size` | `f32` | 字体大小（默认：12.0） |
| `text_color` | `Color` | 文本颜色 |
| `background_color` | `Option<Color>` | 背景填充 |
| `border_color` | `Option<Color>` | 边框颜色 |
| `border_width` | `f32` | 边框宽度 |
| `options` | `Vec<String>` | 列表/组合框选项 |
| `max_length` | `Option<u32>` | 最大文本长度 |
| `is_multiline` | `bool` | 多行文本 |
| `is_password` | `bool` | 密码掩码 |
| `is_spell_check_enabled` | `bool` | 拼写检查 |
| `is_scrollable` | `bool` | 可滚动文本区域 |

### 通过 PdfPage 添加表单字段

```rust
if let Some(page) = doc.get_page(0) {
    // 文本字段
    page.add_text_field("email", Rect::new(100, 700, 200, 20), "user@example.com");

    // 复选框
    page.add_checkbox("subscribe", Rect::new(100, 670, 15, 15), true);

    // 按钮
    page.add_button("submit", Rect::new(100, 620, 120, 30), "提交");

    // 检索所有表单字段
    let fields = page.form_fields();
    for field in &fields {
        println!("字段: {}", field.name);
    }
}
```

### `PdfFormField` 枚举（序列化）

```rust
pub enum PdfFormField {
    TextField { name: String, rect: Rect, value: String },
    CheckBox { name: String, rect: Rect, checked: bool },
    Button { name: String, rect: Rect, text: String },
    // ... 其他变体：radio, list, combo, signature
}
```

---

## 6. 超链接

### `LinkAction` 枚举

```rust
pub enum LinkAction {
    GoToPage { page: u32, x: f32, y: f32 },          // 跳转到页面
    GoToNamedDestination(String),                      // 命名目标
    Uri(String),                                       // 外部 URL
    LaunchFile(String),                                // 启动外部文件
    JavaScript(String),                                // 执行 JS 操作
    NamedAction(NamedAction),                          // 标准操作
}

pub enum NamedAction {
    NextPage, PrevPage, FirstPage, LastPage,
    Print, SaveAs,
}
```

### 创建超链接

```rust
use rust_widgets::pdf::hyperlink::{Hyperlink, LinkAction, HighlightMode, NamedDestination};
use rust_widgets::core::Rect;

// URI 超链接
let link = Hyperlink::new(
    "link-1".into(),
    0,
    Rect::new(100, 500, 200, 20),
    LinkAction::Uri("https://example.com".into()),
)
.with_tooltip("访问我们的网站".into());

// 页面导航
let link = Hyperlink::new(
    "link-2".into(),
    0,
    Rect::new(100, 450, 200, 20),
    LinkAction::GoToPage { page: 3, x: 0.0, y: 0.0 },
)
.with_tooltip("跳转到第 3 页".into());

// 命名操作
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

// 添加链接
manager.add_link(web_link);
manager.add_link(page_link);
println!("链接总数: {}", manager.link_count());

// 命名目标
let dest = NamedDestination::new("chapter-1".into(), 1, 0.0, 0.0)
    .with_zoom(1.5);
manager.add_named_destination(dest);

println!("目标数: {}", manager.destination_count());

// 按页面查询
let page_links = manager.get_page_links(0);
for link in &page_links {
    println!("第 {} 页上的链接: {:?}", link.page, link.action);
}

// 点击测试
if let Some(link) = manager.get_link_at_point(0, 150, 510) {
    println!("已点击: {}", link.tooltip);
}

// 获取命名目标
if let Some(dest) = manager.get_named_destination("chapter-1") {
    println!("跳转到第 {} 页，缩放比例 {}", dest.page, dest.zoom);
}

// 清理
manager.clear();
```

### `LinkBorder` 与 `HighlightMode`

```rust
use rust_widgets::pdf::hyperlink::{LinkBorder, HighlightMode};

let border = LinkBorder {
    horizontal_corner_radius: 2.0,
    vertical_corner_radius: 2.0,
    border_width: 1.0,
    dash_pattern: Some(vec![3.0, 2.0]),  // 虚线边框
};

let link = Hyperlink::new(/* ... */)
    .with_border(border)
    .with_highlight_mode(HighlightMode::Invert);  // 或 None, Outline, Push
```

---

## 7. 元数据与安全

### `PdfMetadata`

```rust
use rust_widgets::pdf::metadata::PdfMetadata;

let metadata = PdfMetadata {
    title: "季度报告".into(),
    author: "李四".into(),
    subject: "2026 年第二季度财务结果".into(),
    keywords: "财务, 季度, 2026, 报告".into(),
    creator: "rust-widgets 1.0.0".into(),
    producer: "rust-widgets PDF 引擎".into(),
    creation_date: "2026-06-10T12:00:00Z".into(),
    modification_date: "2026-06-10T12:00:00Z".into(),
};

doc.set_metadata(metadata);
let meta = doc.metadata();
println!("标题: {}", meta.title);
```

### `PdfSecurity`（诊断用途）

安全选项仅供诊断/文档用途：

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
println("允许打印: {}", sec.allow_print);
```

---

## 8. 基于 SVG 的导出管线（`PdfExporter`）

`PdfExporter` 通过 SVG 中间渲染将 widget 树转换为 PDF，确保像素级精确输出。

### 页面尺寸

```rust
use rust_widgets::pdf::export::PageSize;

// 标准尺寸
let a4 = PageSize::A4.dimensions();        // 595.0 × 842.0 pt
let letter = PageSize::Letter.dimensions(); // 612.0 × 792.0 pt

// 自定义尺寸
let custom = PageSize::Custom { width: 400.0, height: 600.0 };

// 转换为 Size
let size = PageSize::A4.to_size();
println!("A4: {}×{} pt", size.width, size.height);
```

### 方向

```rust
use rust_widgets::pdf::export::PdfOrientation;

let orientation = PdfOrientation::Landscape;
let dims = PageSize::A4.dimensions();
let (width, height) = orientation.apply(dims);
// 横向：width = 842.0, height = 595.0
```

### 导出设置

```rust
use rust_widgets::pdf::export::{PdfExportSettings, PageSize, PdfOrientation};

let settings = PdfExportSettings {
    page_size: PageSize::A4,
    orientation: PdfOrientation::Portrait,
    margins: (20.0, 20.0, 20.0, 20.0),  // 上、右、下、左
    dpi: 96.0,
};

println!("有效区域: {}×{} pt", settings.effective_dimensions().0, settings.effective_dimensions().1);
println!("内容区域: {}×{} pt", settings.content_width(), settings.content_height());
println!("像素尺寸: {}×{} px", settings.pixel_size().0, settings.pixel_size().1);
```

### 导出 Widget

```rust
use rust_widgets::pdf::export::{PdfExporter, PdfExportSettings, PageSize};

// 使用自定义设置创建导出器
let exporter = PdfExporter::with_settings(PdfExportSettings {
    page_size: PageSize::A4,
    ..Default::default()
});

// 将 widget 树导出为 PDF
let widgets: Vec<&dyn Widget> = vec![&root_widget];
exporter.export(&widgets, "report.pdf").expect("导出失败");
```

### 渲染单个页面

```rust
// 从 widget 渲染页面
let pages = exporter.render_pages(&widgets);
for page in &pages {
    println!("第 {} 页: {}×{} pt ({}×{} px)",
        page.index, page.width_pt, page.height_pt,
        page.width_px, page.height_px);
    // page.svg_content 包含该页面的 SVG
}
```

### 一键导出

```rust
use rust_widgets::pdf::export::export_to_pdf;

export_to_pdf(&widgets, "output.pdf").expect("导出失败");
```

---

## 9. 打印系统

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

### 实现 `PrintDocument`

```rust
use rust_widgets::print::{PrintDocument, PrintContext};
use rust_widgets::core::{Rect, Size};

struct InvoiceDocument {
    items: Vec<InvoiceItem>,
}

impl PrintDocument for InvoiceDocument {
    fn page_count(&self) -> u32 {
        // 每页显示 20 项
        ((self.items.len() as u32).saturating_sub(1) / 20 + 1).max(1)
    }

    fn draw_page(&self, page_num: u32, context: &mut dyn PrintContext) {
        let size = context.page_size();

        // 页眉
        context.draw_text("INVOICE", 50.0, 50.0, 18.0);
        context.draw_line(50.0, 60.0, size.width as f32 - 50.0, 60.0, 1.0);

        // 表头
        context.fill_rect(Rect::new(50, 70, (size.width as i32 - 100).max(0) as u32, 20), 0xCCCCCC);
        context.draw_text("Item", 60.0, 82.0, 11.0);
        context.draw_text("Qty", 300.0, 82.0, 11.0);
        context.draw_text("Price", 400.0, 82.0, 11.0);

        // 项目行
        let start = (page_num * 20) as usize;
        let end = ((start + 20).min(self.items.len())) as usize;
        for (i, item) in self.items[start..end].iter().enumerate() {
            let y = 100.0 + (i as f32) * 20.0;
            context.draw_text(&item.name, 60.0, y, 10.0);
            context.draw_text(&item.qty.to_string(), 300.0, y, 10.0);
            context.draw_text(&format!("${:.2}", item.price), 400.0, y, 10.0);
        }

        // 页脚
        let bottom = size.height as f32 - 30.0;
        context.draw_text(&format!("第 {} 页 / 共 {} 页", page_num + 1, self.page_count()),
            50.0, bottom, 9.0);
    }
}
```

---

## 10. `PrintPagination` — 灵活的页码范围 DSL

```rust
use rust_widgets::print::{PrintPagination, PageOrder, PageFilter};

let mut pagination = PrintPagination::new();

// 通过 DSL 设置特定的页码范围
pagination.set_ranges_from_spec("1-3,5,8-10").unwrap();
// 打印第 1,2,3,5,8,9,10 页

// 或设置单个范围
pagination.set_range(1, 5);  // 第 1-5 页

// 添加额外的范围
pagination.add_range(7, 9);   // 额外添加第 7-9 页

// 多份打印，支持逐份打印
pagination.set_copies(3);
pagination.set_collate(true);       // AABBCC vs AAABBBCCC

// 页面顺序
pagination.set_page_order(PageOrder::Descending);

// 奇偶筛选
pagination.set_page_filter(PageFilter::Odd);  // 仅奇数页

// 清除所有显式范围（恢复到"所有页面"）
pagination.clear_ranges();
```

### 页码范围 DSL 示例

| 规格 | 包含的页面 |
|------|----------------|
| `""` | 所有页面 |
| `"1-5"` | 1, 2, 3, 4, 5 |
| `"1,3,5"` | 1, 3, 5 |
| `"1-3,7,9-10"` | 1, 2, 3, 7, 9, 10 |
| `"5-1"` | 1, 2, 3, 4, 5（自动排序） |

---

## 11. `Printer` — 平台后端选择

`Printer` 根据平台选择适当的打印后端：

```rust
use rust_widgets::print::Printer;

let printer = Printer::new();
// 自动检测：Unix 上使用 lp/lpr，Windows 上使用 print

// 使用默认分页打印（所有页面，一份）
printer.print(&my_document);

// 使用自定义分页打印
let mut pagination = PrintPagination::new();
pagination.set_range(1, 5);
pagination.set_copies(2);
printer.print_with_pagination(&my_document, &pagination);

// 带结果检查的打印
match printer.print_with_result(&my_document) {
    Ok(()) => println!("打印任务已成功提交"),
    Err(e) => eprintln!("打印失败: {}", e),
}

// 带分页和结果检查的打印
match printer.print_with_pagination_result(&my_document, &pagination) {
    Ok(()) => println!("分页打印已提交"),
    Err(e) => eprintln!("分页打印失败: {}", e),
}
```

### 打印后端检测

```rust
// Unix: 检查 lp --version 或 lpr --version
// Windows: 检查 print /?
```

---

## 12. `PrintDialog`

```rust
use rust_widgets::print::{PrintDialog, PrintPagination, PageOrder, PageFilter};

let mut dialog = PrintDialog::new();

// 通过对话框配置
dialog.set_copies(2);

// 访问分页设置
dialog.pagination_mut().set_range(1, 10);
dialog.pagination_mut().set_page_order(PageOrder::Ascending);
dialog.pagination_mut().set_collate(true);
dialog.pagination_mut().set_page_filter(PageFilter::All);

// 显示对话框（检查本地打印后台处理程序）
if dialog.show() {
    println!("对话框已接受");
} else {
    eprintln!("没有可用的打印后台处理程序");
}

// 检查对话框是否成功显示
if dialog.was_shown() {
    println!("用户已确认打印");
}
```

---

## 13. `PrintPreviewDialog`

```rust
use rust_widgets::print::PrintPreviewDialog;

let doc = Box::new(MyPrintDocument::new());
let mut preview = PrintPreviewDialog::new(doc);

println!("总页数: {}", preview.page_count());

// 翻页
preview.next_page();
println!("当前页: {}", preview.current_page());

preview.prev_page();
println!("当前页: {}", preview.current_page());

// 显示预览（使用 Memory 后端渲染文档）
if preview.show() {
    let commands = preview.preview_commands();
    println!("预览已生成: {} 条命令", commands.len());
}
```

---

## 14. 带任务生命周期的打印管理器

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
            // 合并对话框分页设置
            let dialog_pagination = self.dialog.pagination().clone();
            self.printer.print_with_pagination_result(doc, &dialog_pagination)
        } else {
            // 使用程序化分页
            self.printer.print_with_pagination_result(doc, &pagination)
        }
    }

    fn preview_document(&self, doc: Box<dyn PrintDocument>) {
        let mut preview = PrintPreviewDialog::new(doc);
        if preview.show() {
            println!("预览就绪: {} 页", preview.page_count());
        }
    }
}
```

---

## 15. 完整发票 PDF 示例

```rust
use rust_widgets::pdf::{PdfWriter, PdfDocument, PdfPage};
use rust_widgets::core::{Color, Rect, Size};

fn generate_invoice() -> Result<(), std::io::Error> {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 595.0, height: 842.0 });

    if let Some(page) = doc.get_page(0) {
        // 公司抬头
        page.draw_text("ACME Corporation", 50.0, 780.0, 22.0,
            Color { r: 33, g: 33, b: 33, a: 255 });
        page.draw_text("123 Business Ave, Suite 100", 50.0, 760.0, 10.0,
            Color { r: 100, g: 100, b: 100, a: 255 });
        page.draw_text("invoice@acmecorp.com", 50.0, 748.0, 10.0,
            Color { r: 100, g: 100, b: 100, a: 255 });

        // 发票标题
        page.draw_text("INVOICE", 400.0, 780.0, 28.0,
            Color { r: 66, g: 133, b: 244, a: 255 });
        page.draw_text("# INV-2026-0042", 400.0, 758.0, 12.0,
            Color { r: 80, g: 80, b: 80, a: 255 });

        // 分隔线
        page.draw_line(50.0, 730.0, 545.0, 730.0, 2.0,
            Color { r: 66, g: 133, b: 244, a: 255 });

        // 客户信息 / 日期
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

        // 表格表头
        let header_bg = Color { r: 66, g: 133, b: 244, a: 255 };
        let header_text = Color { r: 255, g: 255, b: 255, a: 255 };
        page.fill_rect(Rect::new(50, 640, 495, 24), header_bg);
        page.draw_text("Description", 60.0, 648.0, 11.0, header_text);
        page.draw_text("Qty", 320.0, 648.0, 11.0, header_text);
        page.draw_text("Rate", 400.0, 648.0, 11.0, header_text);
        page.draw_text("Amount", 480.0, 648.0, 11.0, header_text);

        // 行项目
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

        // 合计
        page.draw_line(350.0, y, 545.0, y, 1.0,
            Color { r: 150, g: 150, b: 150, a: 255 });
        let total: f64 = items.iter().map(|(_, h, r)| h * r).sum();
        page.draw_text(&format!("Total: ${:.2}", total), 400.0, y - 18.0, 14.0,
            Color { r: 33, g: 33, b: 33, a: 255 });

        // 页脚
        page.draw_line(50.0, 80.0, 545.0, 80.0, 1.0,
            Color { r: 200, g: 200, b: 200, a: 255 });
        page.draw_text("感谢您的惠顾！", 50.0, 65.0, 9.0,
            Color { r: 120, g: 120, b: 120, a: 255 });
    }

    // 页码
    doc.set_page_numbering_enabled(true);
    doc.set_page_numbering_format("Page", 1);
    doc.set_page_numbering_layout(20.0, 20.0, 8.0);

    // 元数据
    doc.set_metadata(PdfMetadata {
        title: "Invoice INV-2026-0042".into(),
        author: "ACME Corporation".into(),
        ..Default::default()
    });

    doc.save("invoice.pdf")
}

fn main() {
    generate_invoice().expect("生成发票 PDF 失败");
    println!("发票已保存到 invoice.pdf");
}
```

---

## 16. 架构总结

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

### 打印管线

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

| 组件 | 角色 |
|-----------|------|
| `PdfWriter` | 工厂：创建文档，检测后端 |
| `PdfDocument` trait | 多页文档管理 |
| `PdfPage` trait | 每页绘图操作符和表单字段 |
| `Annotation` / `AnnotationManager` | 29 种注释类型，带标志 |
| `FormField` / `PdfFormField` | 8 种字段类型，完整序列化 |
| `Hyperlink` / `HyperlinkManager` | URI、页面和命名操作链接 |
| `PdfMetadata` | 文档标题、作者、日期 |
| `PdfSecurity` | 诊断性安全模型 |
| `PdfExporter` | Widget → SVG → PDF 导出管线 |
| `PageSize` / `PdfOrientation` | 标准页面尺寸和方向 |
| `PdfExportSettings` | DPI、边距、页面大小配置 |
| `PrintDocument` trait | 可打印文档接口 |
| `PrintContext` trait | 打印渲染的绘图上下文 |
| `PrintPagination` | 页码范围 DSL、份数、顺序、筛选 |
| `Printer` | 平台特定的打印后端选择 |
| `PrintDialog` | 本地打印对话框集成 |
| `PrintPreviewDialog` | 使用 Memory 后端进行文档预览 |
