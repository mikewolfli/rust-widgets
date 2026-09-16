#!/usr/bin/env python3
"""Split access.rs into per-category .in.rs files using the create_widgets pattern."""

import re

SOURCE = "/home/mikeli/workspace/rust-widgets/src/widget/capability/access.rs"
OUT_DIR = "/home/mikeli/workspace/rust-widgets/src/widget/capability"

# Category definitions: (short_name, [widget_kind_names])
# Order matches the order they appear in the match arms
CATEGORIES = {
    "base": [
        "Button", "Label", "CheckBox", "RadioButton", "Window", "GroupBox",
        "Panel | WidgetKind::Frame", "ToggleButton", "Line",
    ],
    "input": [
        "Slider", "ProgressBar", "ScrollBar", "ListBox", "SpinBox", "ComboBox",
        "Dial", "LCDNumber", "CommandLink", "FontComboBox", "LineEdit",
        "CheckListBox", "Spinner", "Roller", "Dropdown", "TextArea",
        "Keyboard", "Switch", "SearchBar", "ShortcutEditor",
        "CupertinoSlider",
    ],
    "view": [
        "ListView", "TreeView", "Table", "DataView", "PropertyGrid",
        "ImageGallery",
    ],
    "container": [
        "Splitter", "Toolbox", "ScrollArea", "TabWidget", "StackedWidget",
        "CollapsiblePane", "DockWidget", "MdiArea", "TileView",
        "PagerPageView",
    ],
    "dialog": [
        "MessageBox", "FileDialog", "FontDialog", "InputDialog",
        "ProgressDialog", "PopupWindow", "ColorDialog",
    ],
    "menu": [
        "Action", "TabBar", "Menu", "MenuBar", "ToolBar", "RibbonBar",
        "ToolButton", "StatusBar",
    ],
    "advanced": [
        "Calendar", "DatePicker", "TimePicker", "DateTimePicker", "PieMenu",
        "TabView", "MaterialNavigationRail",
    ],
    "media": [
        "AnimatedImage", "HeroAnimation", "LottieWidget", "RiveWidget",
        "VideoPlayer", "AudioVisualizer", "CameraPreview",
    ],
    "other": [
        "RichEdit", "Chart", "TextEdit", "Canvas", "WebEngineView",
        "Grid", "FreeformShape", "Arc", "Meter", "MiniChart",
        "ImageView", "LineChart", "Sparkline", "BarChart", "PieChart",
        "BarcodeScanner", "BezierCurveEditor", "SwipeToDismiss",
    ],
}

def read_file(path):
    with open(path, "r") as f:
        return f.read()

def write_file(path, content):
    with open(path, "w") as f:
        f.write(content)

def find_widgetkind_arms(text, start_marker, end_marker, is_write=False):
    """Find all WidgetKind match arms between start_marker and end_marker.
    
    Returns list of (kind_name, arm_text) tuples.
    In read mode: `WidgetKind::X => match property_name { ... },`
    In write mode: `WidgetKind::X => { if let Some(...) { match property_name { ... } } else { ... } }`
    """
    start = text.index(start_marker)
    end = text.index(end_marker, start)
    body = text[start:end]
    
    arms = []
    # Find all `WidgetKind::... =>` patterns
    pattern = re.compile(r'(WidgetKind::[\w| ]+) =>')
    pos = 0
    while pos < len(body):
        m = pattern.search(body, pos)
        if m is None:
            break
        arm_start = m.start()
        kind_expr = m.group(1).strip()
        # Find the arm end - we need to count braces
        brace_start = m.end()
        # Skip whitespace/newline after =>
        while brace_start < len(body) and body[brace_start] in ' \n\r\t':
            brace_start += 1
        if brace_start >= len(body):
            break
        
        if is_write:
            # Write mode: WidgetKind::X => { ... }
            # Find the matching closing brace for the outer block
            if body[brace_start] == '{':
                depth = 1
                i = brace_start + 1
                while i < len(body) and depth > 0:
                    if body[i] == '{':
                        depth += 1
                    elif body[i] == '}':
                        depth -= 1
                    i += 1
                arm_end = i
            else:
                arm_end = brace_start + 1
        else:
            # Read mode: WidgetKind::X => match property_name { ... },
            # The arm ends at the comma after the closing brace of the inner match
            # Find the match body
            match_idx = body.find("match property_name", brace_start, brace_start + 100)
            if match_idx == -1:
                # Could be a combined arm like Panel | Frame
                # Look for the match after the kind
                match_idx = body.find("match property_name", brace_start)
            if match_idx == -1:
                # Try to find the next arm
                next_m = pattern.search(body, m.end())
                arm_end = next_m.start() if next_m else len(body)
            else:
                # Find the opening brace after match property_name
                brace_pos = body.index('{', match_idx)
                depth = 1
                i = brace_pos + 1
                while i < len(body) and depth > 0:
                    if body[i] == '{':
                        depth += 1
                    elif body[i] == '}':
                        depth -= 1
                    i += 1
                arm_end = i  # After the closing brace
        
        arm_text = body[arm_start:arm_end]
        arms.append((kind_expr, arm_text))
        pos = arm_end
    
    return arms

def categorize_arm(kind_expr, categories):
    """Determine which category a WidgetKind arm belongs to."""
    # Extract all individual kind names
    kinds_in_arm = set()
    for part in kind_expr.replace("WidgetKind::", "").split("|"):
        kinds_in_arm.add(part.strip())
    
    for cat_name, kinds in categories.items():
        for k in kinds:
            # k might be like "Panel | WidgetKind::Frame"
            expected_kinds = set()
            for part in k.replace("WidgetKind::", "").split("|"):
                expected_kinds.add(part.strip())
            if kinds_in_arm == expected_kinds:
                return cat_name
    # Fall back to checking if any kind matches
    for cat_name, kinds in categories.items():
        for k in kinds:
            clean_k = k.replace("WidgetKind::", "").strip()
            if clean_k in kinds_in_arm:
                return cat_name
    print(f"WARNING: Could not categorize: {kind_expr}")
    return "other"

def generate_macro_file(cat_name, read_arms, write_arms):
    """Generate an .in.rs file with read and write macros."""
    lines = []
    lines.append(f"macro_rules! impl_read_access_{cat_name} {{")
    lines.append("    () => {")
    for i, (kind_expr, arm_text) in enumerate(read_arms):
        # Add a comma after each arm except the last one
        text = arm_text
        if i < len(read_arms) - 1:
            text = text.rstrip() + ","
        lines.append(text)
    lines.append("    };")
    lines.append("}")
    lines.append(f"pub(crate) use impl_read_access_{cat_name};")
    lines.append("")
    lines.append(f"macro_rules! impl_write_access_{cat_name} {{")
    lines.append("    () => {")
    for i, (kind_expr, arm_text) in enumerate(write_arms):
        text = arm_text
        if i < len(write_arms) - 1:
            text = text.rstrip() + ","
        lines.append(text)
    lines.append("    };")
    lines.append("}")
    lines.append(f"pub(crate) use impl_write_access_{cat_name};")
    return "\n".join(lines)

def extract_match_body(text, func_marker):
    """Extract the match { ... } body from a function containing 'match widget.kind() {'."""
    fidx = text.index(func_marker)
    midx = text.index("match widget.kind()", fidx)
    # Find the opening brace of match (may have whitespace/newline)
    obrace = text.index('{', midx)
    # Count braces to find end
    depth = 1
    i = obrace + 1
    while i < len(text) and depth > 0:
        if text[i] == '{': depth += 1
        elif text[i] == '}': depth -= 1
        i += 1
    return text[obrace+1:i-1]  # content inside match { ... }

def main():
    text = read_file(SOURCE)
    
    read_marker = "#[cfg(not(feature = \"mini\"))]\npub fn read_widget_property_value("
    write_marker = "#[cfg(not(feature = \"mini\"))]\npub fn write_widget_property_value("
    
    read_inner = extract_match_body(text, read_marker)
    write_inner = extract_match_body(text, write_marker)
    
    # Remove the trailing "_ => ..." catch-all from both
    for sep in ["\n_ => Err", "\n        _ => Err"]:
        idx = read_inner.rfind(sep)
        if idx >= 0:
            read_inner = read_inner[:idx]
            break
    for sep in ["\n_ => Err", "\n        _ => Err"]:
        idx = write_inner.rfind(sep)
        if idx >= 0:
            write_inner = write_inner[:idx]
            break
    
    read_inner = read_inner.strip().rstrip(',')
    write_inner = write_inner.strip().rstrip(',')
    
    # Parse read arms: WidgetKind::X => match property_name { ... },
    read_pattern = re.compile(
        r'(WidgetKind::(?:[\w]+(?:\s*\|\s*WidgetKind::[\w]+)*))\s*=>\s*match property_name'
    )
    
    read_arms_raw = []
    pos = 0
    while pos < len(read_inner):
        m = read_pattern.search(read_inner, pos)
        if m is None:
            break
        kind_expr = m.group(1).strip()
        arm_start = m.start()
        # Find the opening brace after "match property_name"
        after = read_inner[m.end():]
        brace_pos_local = after.index('{')
        depth = 1
        i = brace_pos_local + 1
        while i < len(after) and depth > 0:
            if after[i] == '{': depth += 1
            elif after[i] == '}': depth -= 1
            i += 1
        # i is after the closing brace; also skip trailing comma
        arm_end_global = m.end() + i
        while arm_end_global < len(read_inner) and read_inner[arm_end_global] in ',\n\r\t ':
            arm_end_global += 1
        
        arm_text = read_inner[arm_start:arm_end_global].strip().rstrip(',')
        read_arms_raw.append((kind_expr, arm_text))
        pos = arm_end_global
    
    # Parse write arms: WidgetKind::X => { ... }
    write_pattern = re.compile(
        r'(WidgetKind::(?:[\w]+(?:\s*\|\s*WidgetKind::[\w]+)*))\s*=>\s*\{'
    )
    
    write_arms_raw = []
    pos = 0
    while pos < len(write_inner):
        m = write_pattern.search(write_inner, pos)
        if m is None:
            break
        kind_expr = m.group(1).strip()
        arm_start = m.start()
        brace_start = m.end() - 1  # the {
        depth = 1
        i = brace_start + 1
        while i < len(write_inner) and depth > 0:
            if write_inner[i] == '{': depth += 1
            elif write_inner[i] == '}': depth -= 1
            i += 1
        arm_end = i
        
        arm_text = write_inner[arm_start:arm_end].strip()
        write_arms_raw.append((kind_expr, arm_text))
        pos = arm_end
    
    # Categorize arms
    cat_read_arms = {name: [] for name in CATEGORIES}
    cat_write_arms = {name: [] for name in CATEGORIES}
    
    for kind_expr, arm_text in read_arms_raw:
        cat = categorize_arm(kind_expr, CATEGORIES)
        cat_read_arms[cat].append((kind_expr, arm_text))
    
    for kind_expr, arm_text in write_arms_raw:
        cat = categorize_arm(kind_expr, CATEGORIES)
        cat_write_arms[cat].append((kind_expr, arm_text))
    
    # Generate files
    for cat_name in CATEGORIES:
        ra = cat_read_arms[cat_name]
        wa = cat_write_arms[cat_name]
        content = generate_macro_file(cat_name, ra, wa)
        filepath = f"{OUT_DIR}/access_{cat_name}.in.rs"
        write_file(filepath, content)
        print(f"Created {filepath} ({len(ra)} read arms, {len(wa)} write arms)")

if __name__ == "__main__":
    main()
