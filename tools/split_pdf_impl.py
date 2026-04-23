#!/usr/bin/env python3
"""
Split src/pdf/implementation.rs (1511 lines) into pdf/ sub-modules.
"""

import os, shutil

WORKSPACE = "/Users/mikewolfli/Desktop/workspace/rust-widgets"
SRC = "/tmp/implementation_orig.rs"
DIR = os.path.join(WORKSPACE, "src/pdf")
OLD_MOD = os.path.join(DIR, "mod.rs")

with open(SRC, "r", encoding="utf-8") as f:
    lines = f.readlines()

print(f"Total lines: {len(lines)}")

# Line ranges (0-indexed) based on structure analysis:
# 0-8:    doc comment + imports
# 9-23:   impl Default for PdfMetadata
# 25-51:  PdfSecurity
# 39-50:  impl Default for PdfSecurity
# 52-86:  PdfWriter
# 88-162: PdfReader
# 164-306: PdfDocumentImpl
# 307-464: PdfPageImpl
# 465-497: PdfFormField
# 498-523: PdfPagination
# 524-549: pdf_escape_literal + PdfFontResource
# 550-565: sanitize_pdf_font_name
# 566-769: build_minimal_pdf_bytes + append_page_number_footer
# 770-842: serialize_pdf_form_field_widget + helpers
# 843-908: serialize/parse security + parse helpers
# 910-1000: ParsedPdfPage + parse page functions
# 1000-1057: ImageEncodingRoute + helpers
# 1058-1511: mod tests

def extract(start, end):
    return lines[start:end]

def write_file(name, content_lines):
    path = os.path.join(DIR, name)
    with open(path, "w", encoding="utf-8") as f:
        f.writelines(content_lines)
    print(f"{name}: {len(content_lines)} lines")
    return path

# ============================================================
# types.rs — PdfSecurity, PdfFormField, PdfPagination, PdfFontResource, ImageEncodingRoute, ParsedPdfPage, PdfMetadata
# ============================================================
t = []
t.append("//! PDF data types and structures.\n")
t.append("\n")
t.append("use crate::core::{Color, Rect, Size};\n")
t.append("use std::collections::HashMap;\n")
t.append("\n")
# PdfSecurity (lines 23-49, includes derive + struct + impl Default)
t.extend(extract(23, 50))
t.append("\n")
# PdfFormField (lines 463-495)
t.extend(extract(463, 496))
t.append("\n")
# PdfPagination + Default impl (lines 496-522)
t.extend(extract(496, 523))
t.append("\n")
# PdfFontResource (lines 528-548)
t.extend(extract(528, 549))
t.append("\n")
# ImageEncodingRoute + normalize_image_payload_to_rgb (lines 1006-1055)
t.extend(extract(1006, 1056))
t.append("\n")
# ParsedPdfPage (lines 909-912)
t.extend(extract(909, 913))
t.append("\n")
write_file("types.rs", t)

# ============================================================
# security.rs — serialize/parse PdfSecurity
# ============================================================
sec = []
sec.append("//! PDF security serialization and parsing.\n")
sec.append("\n")
sec.append("use crate::pdf::types::*;\n")
sec.append("\n")
# serialize_security_diagnostics_entries + parse helpers (lines 858-908)
sec.extend(extract(858, 909))
sec.append("\n")
write_file("security.rs", sec)

# ============================================================
# writer.rs — PdfWriter + helpers
# ============================================================
w = []
w.append("//! PDF writer for document generation.\n")
w.append("\n")
w.append("use crate::core::{Color, Rect, Size};\n")
w.append("use crate::pdf::types::*;\n")
w.append("use crate::pdf::document::*;\n")
w.append("use std::collections::HashMap;\n")
w.append("use std::fs;\n")
w.append("use std::io::{Error, ErrorKind};\n")
w.append("use std::path::Path;\n")
w.append("\n")
# PdfWriter (lines 50-85)
w.extend(extract(50, 86))
w.append("\n")
# build_minimal_pdf_bytes + append_page_number_footer (lines 565-768)
w.extend(extract(565, 769))
w.append("\n")
# serialize_pdf_form_field_widget + helpers pdf_rect, pdf_form_field_name (lines 769-857)
w.extend(extract(769, 858))
w.append("\n")
# pdf_escape_literal (lines 523-527)
w.extend(extract(523, 528))
w.append("\n")
# sanitize_pdf_font_name (lines 549-564)
w.extend(extract(549, 565))
w.append("\n")
write_file("writer.rs", w)

# ============================================================
# reader.rs — PdfReader + ParsedPdfPage + parse_* functions
# ============================================================
r = []
r.append("//! PDF reader and parsing logic.\n")
r.append("\n")
r.append("use crate::core::{Rect, Size};\n")
r.append("use crate::pdf::types::*;\n")
r.append("use std::collections::HashMap;\n")
r.append("\n")
# PdfReader (lines 86-161)
r.extend(extract(86, 162))
r.append("\n")
# parse_pdf_pages + parse_pdf_objects + helpers + hex_encode (lines 913-1005)
r.extend(extract(913, 1006))
r.append("\n")
# hex_encode (lines 999-1007)
# (hex_encode is included in extract(913, 1000) range above)
r.append("\n")
write_file("reader.rs", r)

# ============================================================
# document.rs — PdfDocumentImpl + PdfDocument trait impl
# ============================================================
d = []
d.append("//! PDF document implementation.\n")
d.append("\n")
d.append("use crate::core::{Color, Rect};\n")
d.append("use crate::pdf::types::*;\n")
d.append("use crate::pdf::page::PdfPageImpl;\n")
d.append("use std::collections::HashMap;\n")
d.append("\n")
# PdfDocumentImpl (lines 163-305)
# Includes PdfDocumentImpl struct + impl PdfDocumentImpl + impl PdfDocument for PdfDocumentImpl
d.extend(extract(163, 306))
d.append("\n")
write_file("document.rs", d)

# ============================================================
# page.rs — PdfPageImpl + PdfPage trait impl
# ============================================================
p = []
p.append("//! PDF page implementation.\n")
p.append("\n")
p.append("use crate::core::{Color, Rect};\n")
p.append("use crate::pdf::types::*;\n")
p.append("use crate::pdf::document::PdfDocumentImpl;\n")
p.append("use std::collections::HashMap;\n")
p.append("\n")
# PdfPageImpl (lines 306-461)
p.extend(extract(306, 462))
p.append("\n")
write_file("page.rs", p)

# ============================================================
# tests.rs
# ============================================================
tt = []
tt.append("//! PDF tests.\n")
tt.append("\n")
# Include from mod tests line to end (lines 1057-1510)
tt.extend(extract(1057, len(lines)))

with open(os.path.join(DIR, "tests.rs"), "w", encoding="utf-8") as f:
    f.writelines(tt)
print(f"tests.rs: {len(tt)} lines")

# ============================================================
# mod.rs — rewrite
# ============================================================
m = []
m.append("//! PDF generation, parsing, and document manipulation.\n")
m.append("\n")
m.append("pub mod metadata;\n")
m.append("pub mod types;\n")
m.append("pub mod security;\n")
m.append("pub mod writer;\n")
m.append("pub mod reader;\n")
m.append("pub mod document;\n")
m.append("pub mod page;\n")
m.append("\n")
m.append("pub use crate::pdf::types::*;\n")
m.append("pub use crate::pdf::security::*;\n")
m.append("pub use crate::pdf::writer::*;\n")
m.append("pub use crate::pdf::reader::*;\n")
m.append("pub use crate::pdf::document::*;\n")
m.append("pub use crate::pdf::page::*;\n")
m.append("\n")
# tests.rs includes its own `mod tests { ... }` wrapper
# No need for `#[cfg(test)] mod tests;` here

with open(OLD_MOD, "r", encoding="utf-8") as f:
    existing_mod = f.read()

# Check if existing mod.rs already has these declarations
if "pub mod metadata;" not in existing_mod:
    print("WARNING: existing mod.rs is missing pub mod metadata;")
if "mod tests;" in existing_mod:
    print("Note: existing mod.rs already has mod tests;")

with open(OLD_MOD, "w", encoding="utf-8") as f:
    f.writelines(m)
print(f"mod.rs: {len(m)} lines")

# ============================================================
# Remove old implementation.rs
# ============================================================
os.remove(SRC)
print(f"Removed {SRC}")

print("\n✅ PDF split complete!")
