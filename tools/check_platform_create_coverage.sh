#!/usr/bin/env bash
# ============================================================================
# check_platform_create_coverage.sh — 平台控件工厂必须能查到控件（跨层一致性）
# ============================================================================
# 这条门禁的依据（BLUE20 §1.7 / §3.4，规则 #110/#111）：
#
#   **「平台层有一个 create_*」不等于「这个控件能被创建」。**
#
# 第 58 轮取证发现 41 个平台控件工厂里有一个（`checkbox`）在 capability 表里
# 查不到任何东西：控件 `check_box` 本身完整、能构造、有属性、有事件，但只要调用方
# 按**平台层的拼写**去创建，`WidgetFactory::create("checkbox", ..)` 就返回 `None`。
# 这是设计器最先会踩到的坑——平台控件清单与控件注册表对不上，控件就从列表里静默消失。
#
# # 为什么判据必须是「返回 ObjectId 且接受几何参数」
#
# 第一版脚本的判据是「平台层有没有同名 create_* 而 capability 表里没有」。
# 这个判据会**误报**：平台层的 `create_*` 不一定都是控件工厂。
# 典型反例是 `create_web_engine(&self) -> Option<Box<dyn NativeWebEngine>>`——
# 它返回的是原生引擎句柄（给 `src/web/` 驱动用），没有几何、不返回 `id`，
# 把它报成「缺了对应 capability」是假阳性（规则 #110）。
#
# 因此判据收紧为：**接受 `width`/`height` 且返回 `ObjectId`** 才是控件工厂。
# 用 `width`/`height` 而不是 `parent` 作为标志，因为窗口是根、本身没有父：
# `create_window(title, x, y, width, height) -> ObjectId` 也是工厂。
#
# # 为什么它必须进门禁而不是只做一次性脚本
#
# 这条缺口是**纯加法**修好的（给 `check_box` 补 `"checkbox"` 别名）。
# 若不把它变成门禁，下一个人改写别名表时就会静默回归——而回归之后
# 症状是「设计器少一个控件」，没有任何报错。这正是 BLUE18 E-1 记下的
# 「没接进 run_all_gates 的门禁等于没接」。
#
# Exit 0 = 每个平台控件工厂都能解析到 capability 或已声明的别名。
# Exit 1 = 有工厂解析不到（真缺口）。
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if [[ ! -f tools/audit_platform_create_coverage.py ]]; then
  echo "missing tools/audit_platform_create_coverage.py" >&2
  exit 2
fi

# The scan is source-text only, so it needs no toolchain and no budget beyond a
# sanity bound; the bound is still set because principle #58 makes an unbounded
# command a process defect regardless of how fast it usually is.
set +e
OUTPUT="$("$PYTHON" tools/audit_platform_create_coverage.py 2>&1)"
STATUS=$?
set -e

printf '%s\n' "$OUTPUT"

if [[ "$STATUS" -ne 0 ]]; then
  echo ""
  echo "check_platform_create_coverage: FAILED"
  echo "  平台层能创建一个控件，控件注册表却查不到它。"
  echo "  修法（规则 #111）：控件本身完整、只是拼写不可达 ⇒ 补别名（纯加法）；"
  echo "  若该名字不是控件工厂（无几何、不返回 ObjectId）⇒ 改判据，不碰代码（规则 #110）。"
  exit 1
fi

echo "✅ check_platform_create_coverage: every platform control factory resolves to a capability or alias"
