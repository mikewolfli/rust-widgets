#!/usr/bin/env python3
"""
Split control_backend/implementation.rs (2403 lines) into sub-modules.

Strategy (from split1.md):
  - types.rs:       types (ControlBackendKind, ControlRoutePreference, CustomControlState)
  - routing.rs:     route_preference_for_widget_kind
  - trait_def.rs:   ControlBackend trait definition
  - native.rs:      NativeControlBackend + impl ControlBackend
  - custom.rs:      CustomPaintControlBackend + impl ControlBackend + state types
  - dispatcher.rs:  get_control_backend, get_control_backend_for_widget, active_control_policy
  - mod.rs:         re-export hub

Preserve-copy rule: source lines are copied exactly, no simplifications.
"""

import os

BACKEND_DIR = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "src", "control_backend"
)
SOURCE = os.path.join(BACKEND_DIR, "implementation.rs")

with open(SOURCE) as f:
    lines = f.readlines()

total = len(lines)
print(f"Total lines: {total}")

# =========================================================
# 1. types.rs: imports + enums + CustomControlState
#    Lines 1-24 (header) + 895-930 (CustomControlState/CustomWidgetProperties)
# =========================================================
types_content = "".join(lines[0:24]) + "".join(lines[894:930])
# Make types pub(crate) if used cross-module
types_content = types_content.replace(
    "struct CustomControlState {",
    "pub(crate) struct CustomControlState {"
)
types_content = types_content.replace(
    "struct CustomWidgetProperties {",
    "pub(crate) struct CustomWidgetProperties {"
)
# Make all CustomControlState fields pub(crate)
types_content = types_content.replace(
    "    next_widget_id: ObjectId,",
    "    pub(crate) next_widget_id: ObjectId,"
)
types_content = types_content.replace(
    "    texts: HashMap<ObjectId, String>,",
    "    pub(crate) texts: HashMap<ObjectId, String>,"
)
types_content = types_content.replace(
    "    enabled: HashMap<ObjectId, bool>,",
    "    pub(crate) enabled: HashMap<ObjectId, bool>,"
)
types_content = types_content.replace(
    "    visible: HashMap<ObjectId, bool>,",
    "    pub(crate) visible: HashMap<ObjectId, bool>,"
)
types_content = types_content.replace(
    "    ime_enabled: HashMap<ObjectId, bool>,",
    "    pub(crate) ime_enabled: HashMap<ObjectId, bool>,"
)
types_content = types_content.replace(
    "    accessibility_names: HashMap<ObjectId, String>,",
    "    pub(crate) accessibility_names: HashMap<ObjectId, String>,"
)
types_content = types_content.replace(
    "    menu_trigger_queue: VecDeque<ObjectId>,",
    "    pub(crate) menu_trigger_queue: VecDeque<ObjectId>,"
)
types_content = types_content.replace(
    "    widget_trigger_queue: VecDeque<WidgetTriggerEvent>,",
    "    pub(crate) widget_trigger_queue: VecDeque<WidgetTriggerEvent>,"
)
types_content = types_content.replace(
    "    widget_properties: HashMap<ObjectId, CustomWidgetProperties>,",
    "    pub(crate) widget_properties: HashMap<ObjectId, CustomWidgetProperties>,"
)
# Make all CustomWidgetProperties fields pub(crate)
types_content = types_content.replace(
    "    parent: Option<ObjectId>,",
    "    pub(crate) parent: Option<ObjectId>,"
)
types_content = types_content.replace(
    "    x: i32,",
    "    pub(crate) x: i32,",
)
types_content = types_content.replace(
    "    y: i32,",
    "    pub(crate) y: i32,",
)
types_content = types_content.replace(
    "    width: u32,",
    "    pub(crate) width: u32,",
)
types_content = types_content.replace(
    "    height: u32,",
    "    pub(crate) height: u32,",
)
types_content = types_content.replace(
    "    widget_kind: WidgetKind,",
    "    pub(crate) widget_kind: WidgetKind,",
)
with open(f"{BACKEND_DIR}/types.rs", "w") as f:
    f.write(types_content)
print(f"types.rs: {len(types_content.splitlines())} lines")

# =========================================================
# 2. routing.rs: route_preference_for_widget_kind (lines 25-103)
# =========================================================
routing_content = "use crate::control_backend::types::ControlRoutePreference;\nuse crate::widget::WidgetKind;\n" + "".join(lines[24:103])
with open(f"{BACKEND_DIR}/routing.rs", "w") as f:
    f.write(routing_content)
print(f"routing.rs: {len(routing_content.splitlines())} lines")

# =========================================================
# 3. trait_def.rs: ControlBackend trait (lines 104-468)
# =========================================================
trait_content = "use crate::control_backend::types::ControlBackendKind;\nuse crate::core::ObjectId;\nuse crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};\n" + "".join(lines[103:468])
with open(f"{BACKEND_DIR}/trait_def.rs", "w") as f:
    f.write(trait_content)
print(f"trait_def.rs: {len(trait_content.splitlines())} lines")

# =========================================================
# 4. native.rs: NativeControlBackend + impl (lines 469-894)
# =========================================================
native_content = """use crate::control_backend::types::ControlBackendKind;
use crate::control_backend::trait_def::ControlBackend;
use crate::core::ObjectId;
use crate::platform::{get_platform, WidgetTriggerEvent, WidgetTriggerKind};
""" + "".join(lines[468:894])
with open(f"{BACKEND_DIR}/native.rs", "w") as f:
    f.write(native_content)
print(f"native.rs: {len(native_content.splitlines())} lines")

# =========================================================
# 5. custom.rs: CustomPaintControlBackend + impl (lines 930-2335)
# =========================================================
custom_content = """use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use crate::control_backend::types::{ControlBackendKind, CustomControlState, CustomWidgetProperties};
use crate::control_backend::trait_def::ControlBackend;
use crate::core::ObjectId;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
use crate::widget::WidgetKind;
""" + "".join(lines[930:2331])
with open(f"{BACKEND_DIR}/custom.rs", "w") as f:
    f.write(custom_content)
print(f"custom.rs: {len(custom_content.splitlines())} lines")

# =========================================================
# 6. dispatcher.rs: router functions (lines 2332-2403)
# =========================================================
dispatcher_content = """#[cfg(feature = "controls-custom")]
use std::sync::OnceLock;
#[cfg(feature = "controls-custom")]
use crate::control_backend::custom::CustomPaintControlBackend;
#[cfg(feature = "controls-native")]
use crate::control_backend::native::NativeControlBackend;
use crate::control_backend::trait_def::ControlBackend;
use crate::control_backend::types::{ControlBackendKind, ControlRoutePreference};
use crate::control_backend::routing::route_preference_for_widget_kind;
use crate::widget::WidgetKind;
""" + "".join(lines[2331:])
with open(f"{BACKEND_DIR}/dispatcher.rs", "w") as f:
    f.write(dispatcher_content)
print(f"dispatcher.rs: {len(dispatcher_content.splitlines())} lines")

# =========================================================
# 7. mod.rs: re-export hub
# =========================================================
mod_content = """//! Control backend abstraction for native and custom-painted control paths.
//!
//! AUTO-GENERATED by tools/split_control_backend.py — do not edit manually.
//! Generated from implementation.rs.

pub mod types;
pub mod routing;
pub mod trait_def;
pub mod native;
pub mod custom;
pub mod dispatcher;

// Re-export key public types for backward compatibility.
pub use types::{ControlBackendKind, ControlRoutePreference};
pub use routing::route_preference_for_widget_kind;
pub use trait_def::ControlBackend;
pub use native::NativeControlBackend;
pub use dispatcher::{get_control_backend, get_control_backend_for_widget, active_control_policy};
#[cfg(feature = "controls-custom")]
pub use custom::CustomPaintControlBackend;
"""
with open(f"{BACKEND_DIR}/mod.rs", "w") as f:
    f.write(mod_content)
print(f"mod.rs: {len(mod_content.splitlines())} lines (rewritten)")

# =========================================================
# Summary
# =========================================================
print("\n=== SPLIT COMPLETE ===")
for fname in os.listdir(BACKEND_DIR):
    if fname.endswith(".rs"):
        fpath = os.path.join(BACKEND_DIR, fname)
        with open(fpath) as f:
            flines = f.readlines()
        print(f"  {fname}: {len(flines)} lines")
