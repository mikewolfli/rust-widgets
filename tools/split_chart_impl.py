#!/usr/bin/env python3
"""
Split src/chart/implementation.rs (1593 lines) into chart/ sub-modules.
"""

import os, shutil

WORKSPACE = "/Users/mikewolfli/Desktop/workspace/rust-widgets"
SRC = os.path.join(WORKSPACE, "src/chart/implementation.rs")
BACKUP = os.path.join(WORKSPACE, "src/chart/implementation.rs.bak")
DIR = os.path.join(WORKSPACE, "src/chart")
OLD_MOD = os.path.join(DIR, "mod.rs")

with open(SRC, "r", encoding="utf-8") as f:
    lines = f.readlines()

print(f"Total lines: {len(lines)}")
shutil.copy2(SRC, BACKUP)
print(f"Backed up to {BACKUP}")

# Read existing mod.rs if it exists
existing_mod = ""
if os.path.exists(OLD_MOD):
    with open(OLD_MOD, "r", encoding="utf-8") as f:
        existing_mod = f.read()

# Line ranges (0-indexed):
# 1-20:  doc comment
# 21-22: use imports
# 23-44: DataPoint, ChartSeries, ChartType
# 45-65: MemoryChartContext, SvgChartContext
# 66-93: impl SvgChartContext
# 94-139: impl ChartContext for SvgChartContext
# 140-151: render_chart_to_svg_file
# 152-171: impl ChartContext for MemoryChartContext
# 172-188: pub trait Chart
# 189-199: pub trait ChartContext
# 200-218: LineChart struct, CartesianLayout
# 219-455: cartesian helpers + LineChart impls
# 455-586: impl Chart for LineChart
# 587-626: BarChart struct + impl
# 628-757: impl Chart for BarChart
# 758-775: PieChart struct + impl
# 777-820: impl Chart for PieChart
# 821-866: ScatterChart struct + impls
# 868-993: impl Chart for ScatterChart
# 994-1039: AreaChart struct + impls
# 1041-1172: impl Chart for AreaChart
# 1173-1184: svg helper functions
# 1185-1593: mod tests

# Extract ranges
def extract(start, end):
    return lines[start:end]

# ============================================================
# types.rs — DataPoint, ChartSeries, ChartType, ChartContext, Chart
# ============================================================
t = []
t.append("//! Chart data types, series, chart types, and core traits.\n")
t.append("\n")
t.append("use crate::core::{Color, Point, Rect};\n")
t.append("\n")
t.extend(extract(22, 54))   # DataPoint, ChartSeries, ChartType
t.append("\n")
t.extend(extract(171, 199)) # Chart trait + ChartContext trait
t.append("\n")

with open(os.path.join(DIR, "types.rs"), "w", encoding="utf-8") as f:
    f.writelines(t)
print(f"types.rs: {len(t)} lines")

# ============================================================
# svg.rs — SvgChartContext, MemoryChartContext, render fn, svg helpers
# ============================================================
s = []
s.append("//! SVG chart context and in-memory chart context implementations.\n")
s.append("\n")
s.append("use crate::chart::types::*;\n")
s.append("use crate::core::{Color, Point, Rect};\n")
s.append("use std::fs;\n")
s.append("\n")
s.extend(extract(54, 171))  # MemoryChartContext, SvgChartContext, impls, render fn
s.append("\n")
s.extend(extract(1172, 1183)) # svg helper functions (fn svg_color_hex + helpers, no tests)
s.append("\n")

with open(os.path.join(DIR, "svg.rs"), "w", encoding="utf-8") as f:
    f.writelines(s)
print(f"svg.rs: {len(s)} lines")

# ============================================================
# charts.rs — all chart types (Line, Bar, Pie, Scatter, Area)
# ============================================================
ch = []
ch.append("//! Chart implementations: Line, Bar, Pie, Scatter, Area.\n")
ch.append("\n")
ch.append("use crate::chart::types::*;\n")
ch.append("use crate::chart::svg::*;\n")
ch.append("use crate::chart::layout::*;\n")
ch.append("use crate::core::{Color, Point, Rect};\n")
ch.append("\n")
# LineChart
ch.extend(extract(200, 587))
ch.append("\n")
# BarChart
ch.extend(extract(587, 758))
ch.append("\n")
# PieChart
ch.extend(extract(758, 821))
ch.append("\n")
# ScatterChart
ch.extend(extract(821, 994))
ch.append("\n")
# AreaChart
ch.extend(extract(994, 1172))  # AreaChart (stop before svg helper fn)
ch.append("\n")

with open(os.path.join(DIR, "charts.rs"), "w", encoding="utf-8") as f:
    f.writelines(ch)
print(f"charts.rs: {len(ch)} lines")

# ============================================================
# layout.rs — CartesianLayout + compute_cartesian_layout + draw helpers
# ============================================================
l = []
l.append("//! Cartesian layout computation and axis/legend drawing.\n")
l.append("\n")
l.append("use crate::chart::types::*;\n")
l.append("use crate::chart::svg::*;\n")
l.append("use crate::core::{Color, Point, Rect};\n")
l.append("\n")
l.extend(extract(210, 423))  # CartesianLayout + compute + draw_axes/ticks/legend + truncate (stop before LineChart impl)
l.append("\n")

with open(os.path.join(DIR, "layout.rs"), "w", encoding="utf-8") as f:
    f.writelines(l)
print(f"layout.rs: {len(l)} lines")

# ============================================================
# tests.rs
# ============================================================
tt = []
tt.append("//! Chart tests.\n")
tt.append("\n")
tt.append("#[cfg(test)]\n")
tt.append("mod tests {\n")
tt.extend(extract(1185, len(lines)))
tt.append("}\n")

with open(os.path.join(DIR, "tests.rs"), "w", encoding="utf-8") as f:
    f.writelines(tt)
print(f"tests.rs: {len(tt)} lines")

# ============================================================
# mod.rs — rewrite
# ============================================================
m = []
m.append("//! Chart widgets and drawing contracts.\n")
m.append("\n")
m.append("pub mod types;\n")
m.append("pub mod svg;\n")
m.append("pub mod layout;\n")
m.append("pub mod charts;\n")
m.append("\n")
m.append("pub use crate::chart::types::*;\n")
m.append("pub use crate::chart::svg::*;\n")
m.append("pub use crate::chart::charts::*;\n")
m.append("\n")
m.append("#[cfg(test)]\n")
m.append("mod tests;\n")

with open(OLD_MOD, "w", encoding="utf-8") as f:
    f.writelines(m)
print(f"mod.rs: {len(m)} lines")

# ============================================================
# Remove old implementation.rs
# ============================================================
os.remove(SRC)
print(f"Removed {SRC}")

# Check if chart module is declared in lib.rs
LIB_RS = os.path.join(WORKSPACE, "src/lib.rs")
with open(LIB_RS, "r", encoding="utf-8") as f:
    lib = f.read()
if "pub mod chart;" in lib:
    print("lib.rs: `pub mod chart;` found — no change needed.")
else:
    print("WARNING: `pub mod chart;` not found in lib.rs!")

print("\n✅ Split complete!")
