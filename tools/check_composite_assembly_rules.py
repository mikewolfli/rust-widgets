#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Every composite control must assemble its children the way BLUE22 §B.6 requires.

# Why this gate exists

`src/widget/composite.rs` was built so that a composite control *asks* a layout where its children
go, instead of computing their rectangles by hand. That refactor reached eleven controls
(`split_button`, `spin_box`, `combo_box`, `group_box`, `tool_bar`, `menu`, `status_bar`,
`tab_widget`, `scroll_area`, `tab_view`, `message_box`), and the module it produced is complete:
`add` / `add_sized` / `add_flexible` / `add_with_hints`, `hints()`, `arrange()`, an `invalidate()`
dirty flag.

What was **not** built is anything that keeps it that way. §B.6 lists nine rules; before this gate,
`grep -rn "B.6" tools/` returned nothing, so every one of them was a convention. That is exactly
the shape the original defect had: hand-computed geometry was not wrong on purpose, it was simply
never checked, and the same "icon + gap + label" arithmetic got rewritten in eleven files before
anyone noticed.

# What is checkable, and what is not

Two rules of the nine are lexical and mechanical, and this gate enforces them:

  [1] **Children come from `WidgetFactory::create`.** §B.6 rule 1. A composite that writes
      `Button::new(..)` couples itself to a concrete type, and that type may not exist in a
      stripped profile — the crate already carries `TOGGLE_BUTTON_KIND` and friends for this
      reason. The check is a call to the factory, by one of its two spellings.

  [2] **Children are positioned by a `Layout`.** §B.6 rule 2. A composite that places a child at
      `rect.x + k` loses the layout's device scaling: `update_with_context` grows a layout's gaps
      with the text-size preference, and a hand-written constant does not, so a control's own
      padding scales with the font while its gaps do not. The check is that a *placing* composite
      reaches a layout through one of the four entry points.

# What this gate deliberately does not attempt

Stated so the coverage is not over-read:

  * **Rules 3-10 are semantic and stay the reviewer's.** "Hints propagate upward", "the dirty flag
    causes a re-arrange", "a 4 px child gets a 48 px touch target" are properties of *behaviour*.
    They have unit tests (`composite.rs`'s own suite and the per-control tests); a lexical gate
    asserting them would assert the spelling of the fix rather than the fix.
  * **A composite with no children is out of scope.** The membership test is behavioural, not a
    filename list: a control counts as a composite only if it puts child widgets on the tree
    (`add_child`, or a builder that does it for it). `dialog_widget` hosts *one* caller-supplied
    content widget and has no arrangement of its own to derive; `number_picker` draws its own rows
    and owns no children at all. Neither is a composite in §B.6's sense, and flagging them would be
    a finding about a control that has nothing to fix.
  * **Blanket `Rect::new` arithmetic is not matched.** A control's own *painting* is full of
    legitimate offsets (`rect.x + width` is a right edge, `pos.x < rect.x + rect.width` is a
    hit-test). What the gate looks for is the narrower shape — a rectangle derived from another
    rectangle's origin plus a literal — and it looks for it in the function that hands children to
    the host, not in `draw`.
  * **A `Layout` used for a composite's *own* geometry is not this gate's business.** `tab_bar`
    and `stepper` compute their own rects; the rule is about placing children.

Usage: python3 tools/check_composite_assembly_rules.py
"""

from __future__ import annotations

import pathlib
import re
import sys

# Every composite control §B.8 names, with the file that holds it. Kept as an explicit list rather
# than a glob so that the gate's coverage is a *statement*: the reverse injections below prove the
# list is really read, and a control that graduates into this family is added here on purpose.
#
# # Why `message_box` is absent though §B.8 lists it
#
# §B.8 names eleven controls, but the list is a *plan* written before the work: it says which
# controls have hand-computed geometry, not which ones own child widgets. `message_box` computes a
# button row and paints and hit-tests it itself — it puts no child on the tree at all. Laid out by a
# `Layout` it already is (`action_row_geometry` hands `ChildInfo` hints to a `FlexLayout` with
# `justify_content: FlexEnd`), but the rectangles that come back describe **paint**, not placed
# children, so rules 1 and 2 — which are about children — have nothing to say about it. Listing it
# here would make the gate's membership rule skip it silently and a reader would count it as
# covered. It is therefore named in this comment rather than in the tuple, and the count below is
# the count of real composites.
COMPOSITES: tuple[tuple[str, str], ...] = (
    ("split_button", "src/widget/special_widgets/split_button.rs"),
    ("spin_box", "src/widget/input_widgets/spinbox.rs"),
    ("combo_box", "src/widget/input_widgets/combobox.rs"),
    ("group_box", "src/widget/container_widgets/groupbox.rs"),
    ("tool_bar", "src/widget/menu_toolbar/tool_bar.rs"),
    ("menu", "src/widget/menu_toolbar/menu.rs"),
    ("status_bar", "src/widget/menu_toolbar/status_bar.rs"),
    ("tab_widget", "src/widget/container_widgets/tabwidget.rs"),
    ("scroll_area", "src/widget/container_widgets/scrollarea.rs"),
    ("tab_view", "src/widget/nav_widgets/tab_view.rs"),
)

COMPOSITE_BUILDER = "src/widget/composite.rs"

# Rule 1: the two spellings that reach the capability registry.
FACTORY_CALL = re.compile(r"WidgetFactory::new_with_defaults\(\)|factory\.create\(|\badd_factory\b")
# The same rule, negative: a concrete `Type::new(` construction of a widget.
CONCRETE_CTOR = re.compile(
    r"\b(Button|Label|CheckBox|RadioButton|LineEdit|ComboBox|ListBox|Slider|ProgressBar|Panel|"
    r"SpinBox|Icon|Switch|ToolButton|MenuItem|ToolBar|StatusBar)\s*::\s*new\s*\("
)
# Rule 2: anything that hands children to a layout.
LAYOUT_ENTRY = re.compile(
    r"CompositeBuilder::new|\.arrange\(|\.add_widget\(|Layout::arrange\(\s*|layout\.add_widget\("
)
# Rule 9: the band an `arrange` is handed, so the check is about the *argument* rather than about
# any invented rect in the function.
ARRANGES_INTO = re.compile(r"\.arrange\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)", re.DOTALL)

# Rule 9: an invented rect whose extent is a bare identifier rather than a literal.
#
# The optional `as <ty>` matters: `total as u32` is how the defect was actually spelled in
# `tab_view` (`total + TAB_SPACING * n` with a cast to the rect's `u32`), so a pattern that stopped
# at the identifier would have missed the very case it was written for. A literal extent does
# **not** match — a fixed-size chrome rect is not derived from the children and cannot silently
# absorb a shortfall.
INVENTED_BAND = re.compile(
    r"Rect::new\(\s*0\s*,\s*0\s*,\s*[A-Za-z_][A-Za-z0-9_.]*\s*(?:as\s+[A-Za-z_][A-Za-z0-9_]*\s*)?[,)]"
)


def production_part(text: str) -> str:
    """`text` with every `#[cfg(test)]` module removed.

    # Why this is not optional

    A composite's own test module constructs the control directly — `SpinBox::new(Rect::new(..))`
    is how a test gets a subject to assert on. Rule 1 forbids that *for a child the composite
    creates*, not for a test's own fixture, and the distinction is not cosmetic: without this the
    gate reports four findings that are all about test code, and a gate whose findings are mostly
    noise is a gate that gets skimmed.

    The split is at the first line that is exactly `#[cfg(test)]` at module indentation, which is
    how every file in this crate spells it. A file with an inline `#[cfg(test)]` on a single item
    keeps that item, which errs toward *reporting* rather than hiding — the safe direction for a
    check whose misses are silent.
    """
    for marker in ("\n#[cfg(test)]\n", "\n    #[cfg(test)]\n"):
        if marker in text:
            return text.split(marker, 1)[0]
    return text


# A `Rect::new(..)` whose first argument is `NAME.x + <literal>` and whose *width* is a literal.
#
# # Why the width matters
#
# The §B.6 rule-2 defect is a **child's rectangle computed by hand**: a widget is placed at an
# offset from a sibling and given a size the composite decided. A hover highlight inside `draw` is
# the same arithmetic and none of the same thing — `Rect::new(row.x + 2, row.y, row.width - 4, ..)`
# insets the row it is painting, and the row is not a child. Requiring the width to be a literal
# separates the two without needing to know which function the call sits in: a placed child gets
# both an offset *and* a size from the composite, whereas an inset borrows the extent it started
# from. The earlier form of this pattern matched the inset and produced a finding about painting.
PLACEMENT_LITERAL = re.compile(
    r"Rect::new\(\s*[A-Za-z_][A-Za-z0-9_.]*\.(?:x|y)\s*\+\s*[0-9]+\s*,\s*"
    r"[^,]+,\s*[0-9]+\s*,"
)
# A child put on the tree. This is the membership test for "is a composite".
HAS_CHILDREN = re.compile(r"\.add_child\(|CompositeBuilder::new|builder\.add")

# Reverse-injection target: the line in `split_button`'s production code that hands the children to
# a layout. `--inject` replaces it with hand-computed placement and requires the gate to notice, so
# a check that silently stopped reading its input cannot pass.
INJECTION_TARGET = "builder.arrange(band, &mut |_, rect| placed.push(rect));"


def is_composite(text: str) -> bool:
    """Whether this file assembles child widgets at all. See the header for the membership rule."""
    return bool(HAS_CHILDREN.search(text))


def check_rule_1_factory_creation(path: pathlib.Path, text: str, name: str) -> list[str]:
    """[1] A composite's children must come from `WidgetFactory::create`."""
    findings: list[str] = []
    if not FACTORY_CALL.search(text):
        findings.append(
            f"{name}: {path} assembles children but never reaches `WidgetFactory`, so its children "
            f"are concrete types that a stripped profile may not compile (§B.6 rule 1)"
        )
    concrete = CONCRETE_CTOR.search(text)
    if concrete:
        line = text[: concrete.start()].count("\n") + 1
        findings.append(
            f"{name}: {path}:{line} constructs `{concrete.group(0).strip()}` directly. §B.6 rule 1 "
            f"requires every child to come from `WidgetFactory::create`, or the control is absent "
            f"from `mini`/`embedded` while this file still names it"
        )
    return findings


def check_rule_2_layout_placement(path: pathlib.Path, text: str, name: str) -> list[str]:
    """[2] A composite's children must be positioned by a `Layout`, not by arithmetic."""
    findings: list[str] = []
    if not LAYOUT_ENTRY.search(text):
        findings.append(
            f"{name}: {path} assembles children but never reaches a `Layout` "
            f"(`CompositeBuilder::arrange`, `Layout::arrange`, or `add_widget`), so their "
            f"rectangles come from arithmetic (§B.6 rule 2)"
        )
    for match in PLACEMENT_LITERAL.finditer(text):
        line = text[: match.start()].count("\n") + 1
        findings.append(
            f"{name}: {path}:{line} derives a child rectangle from another rectangle's origin "
            f"plus a literal (`{match.group(0).strip()}`). §B.6 rule 2 requires a `Layout` to "
            f"place it, or the offset does not scale with `layout_scale`/`font_scale`"
        )
    return findings


def check_builder_still_offers_the_channel() -> list[str]:
    """The builder itself must keep the four things a composite depends on."""
    findings: list[str] = []
    path = pathlib.Path(COMPOSITE_BUILDER)
    if not path.exists():
        return [f"{COMPOSITE_BUILDER} is missing; every composite's assembly path is gone"]
    text = path.read_text()
    for method, why in (
        ("pub fn add(", "children created through the factory (§B.6 rule 1)"),
        ("pub fn add_sized(", "a column of chrome whose size is the composite's (§B.6 rule 9)"),
        ("pub fn add_flexible(", "a child that keeps its own extent on one axis (§B.6 rule 9)"),
        ("pub fn hints(", "intrinsic size propagating upward (§B.6 rule 3)"),
        ("pub fn arrange(", "the layout placing the children (§B.6 rule 2)"),
        ("pub fn invalidate(", "a hint change marking the arrangement dirty (§B.6 rule 10)"),
    ):
        if method not in text:
            findings.append(f"`CompositeBuilder::{method}` is gone, so {why} has no channel")
    return findings


def check_one_hints_channel() -> list[str]:
    """A composite must not read a child's size by a second route.

    # Why this is a rule

    §B.6 rule 3 is that a composite's size is derived from its children's `Hints`. `Widget::hints`
    is that channel, and `CompositeBuilder::add` reads it at creation. A composite that instead
    reached for a per-kind size method would have two sources for one fact — which is the shape
    this crate has already paid for once (`set_child_sizes`, removed in the same refactor).
    """
    findings: list[str] = []
    for name, path_str in COMPOSITES:
        text = pathlib.Path(path_str).read_text()
        if "set_child_sizes" in text:
            findings.append(
                f"{name}: {path_str} calls `set_child_sizes`, the pre-hints channel that let a "
                f"layout be told sizes instead of asking for them (§B.6 rule 3)"
            )
    return findings


def check_rule_9_band_is_the_callers_room() -> list[str]:
    """[9] A composite must lay its children out in the room it was **given**, not in a band
    derived from the children's own total.

    # The defect this catches

    §B.6 rule 9 is that `min` and `fill` are separate declarations, and the whole point of
    asking a layout is that it answers "what fits in the room I have?". A composite that
    computes its arranging rect from the sum of the very sizes it is about to hand the layout
    has not asked that question at all — it has handed the layout a band that fits *by
    construction*, so `compute_main_sizes` always sees `remaining >= 0` and the shrink pass can
    never run. Every `min` the children declare is then unobservable, and a strip that cannot
    pay for its content silently overhangs its own control instead of compressing.

    This is the §B.5.1 shape verbatim: the hand-rolled accumulator was not removed, it was
    **moved** from `rect.x + i * width` into the rect's *width*. It is also why this gate had
    to grow a rule: rules 1 and 2 are satisfied by any code that reaches the factory and the
    layout, and this defect satisfies both while undermining the reason to reach them.

    # The two shapes found in this crate

    * `tab_view`: `let run = Rect::new(0, 0, total + TAB_SPACING * n, height)` where `total` is
      the sum of the same `widths` the loop registers.
    * `menu`: `column.arrange(Rect::new(0, 0, width, self.popup_height()), ..)` where
      `popup_height()` is the sum of the row heights the loop just registered.

    Both are matched by looking for a `Rect::new(0, 0, ..)` (a rect with no source rectangle —
    an *invented* band) whose width or height argument is an identifier, **and whose construction
    never mentions `geometry()`**, and which is then passed to `arrange`. A composite that
    arranges into a rect it was handed (`band`, `strip`, `rect`, `content_row`) passes, and so
    does one that derives its band from the control's own rectangle.

    # What this deliberately does not attempt

    Two shapes are out of scope, both because they are not the defect:

    * A rect the composite genuinely owns — `scroll_area`'s
      `Rect::new(0, 0, width, content_row_h)` derives a *fallback* for a child the layout did not
      report, not the band it arranges in — is not passed to `arrange`.
    * `tab_widget`'s `Rect::new(0, 0, extent, TAB_HEIGHT)`. It *is* an invented rect holding an
      identifier, and it *is* arranged into, but it is **not** a defect: `tab_width_hints()`
      compares the tabs' total against `rect`, the control's own real extent, and when they do not
      fit computes an explicit `share` and declares *that* as the triple. The shortfall is known
      to the control before the band is built, so the invented extent is the arithmetic that
      follows the fit decision rather than one that replaces it. `tab_view`'s defect was the
      opposite order: it never compared against anything, and the band encoded the answer.
      The `geometry()` exclusion is exactly this distinction, and it is a *lexical* proxy for it —
      narrow, so the check errs toward reporting.
    """
    findings: list[str] = []
    for path in sorted(pathlib.Path("src/widget").rglob("*.rs")):
        text = production_part(path.read_text())
        if "CompositeBuilder::new" not in text:
            continue
        # The search is scoped to the enclosing function body rather than to a fixed window after
        # `.arrange(`, because the band is an argument to `arrange` only half the time: `tab_view`
        # built it in a `let` several lines above. A 220-character window seemed generous and was
        # not — it stopped one line short of the binding, and the injection below proved it.
        #
        # A "function body" is approximated as "from the nearest preceding `fn ` to the `arrange`
        # call", which is what makes the scope a statement about the code rather than about the
        # gate's window size. It errs toward *reporting*: a false positive is visible and cheap to
        # argue with, whereas a window that is one line too small is silent.
        for match in re.finditer(r"\.arrange\s*\(", text):
            body_start = text.rfind("fn ", 0, match.start())
            body_start = body_start if body_start >= 0 else 0
            bound = ARRANGES_INTO.search(text, match.start())
            if not bound:
                continue
            # The *name* of the rect handed to `arrange`. A rect built inline at the call site
            # (`arrange(Rect::new(0, 0, total, h), ..)`) has no name; the search then runs from the
            # call itself so the inline form is still seen.
            name = bound.group(1)
            inline = text[match.start() : match.end() + 40]
            bound_at = text.rfind(f"let {name} = ", body_start, match.start())
            if bound_at < 0:
                # Not a named binding: only the inline spelling can be the defect.
                found = INVENTED_BAND.search(inline)
                if not found:
                    continue
                line = text[: match.start()].count("\n") + 1
                spelling = found.group(0).strip()
            else:
                # # Why the fit has to be decided *in this function*
                #
                # The distinguishing question is not "does a rectangle appear", and not "does
                # the word `geometry()` appear somewhere in the file" — it is whether **the band
                # is the room the caller supplied**. Two shapes pass that test and one does not:
                #
                # * A band that names the room (`band`, `strip`, `rect`, `content_row`) — table
                #   stakes, handled by the `bound_at < 0` branch above.
                # * A band derived from the control's own rectangle **in this function**. Nothing
                #   else in the body can produce the room, so its presence proves the shortfall was
                #   measured here.
                #
                # The shape that fails is the whole point of the rule: `tab_view`'s band was
                # `Rect::new(0, 0, total + gaps, h)` with no reference to anything the control was
                # given, so the layout could never see a shortfall.
                #
                # # Why `tab_widget` passes, and it is not a loophole
                #
                # Its band is `Rect::new(0, 0, extent, TAB_HEIGHT)` — invented, holding an
                # identifier — but the band is only the *assembly's own coordinate frame*. The fit
                # decision happens one level up and **is** measured against the room:
                # `tab_width_hints()` compares the tabs' total against `rect`, the control's real
                # extent, and when they do not fit computes an explicit `share` and declares that
                # as the triple. The shortfall was therefore already resolved; the invented extent
                # is the arithmetic that *follows* the fit decision instead of replacing it, and
                # the run is translated into the control afterwards. That is the opposite order
                # from `tab_view`, which is the defect.
                #
                # The proxy is deliberately lexical and narrow: the metric that the distinction
                # rests on — "was the shortfall compared against the caller's room?" — is a data
                # flow question this gate cannot answer without building a symbol table, so it
                # asks the nearest checkable question instead. Being narrow is what makes a
                # false negative possible; being broad made the check report `tab_widget`'s
                # already-fixed design, which is worse.
                production = text[body_start:match.start()]
                if "geometry()" in production:
                    continue
                found = INVENTED_BAND.search(text[bound_at : match.start()])
                if not found:
                    continue
                line = text[: found.start() + bound_at].count("\n") + 1
                spelling = found.group(0).strip()
            findings.append(
                f"{path.as_posix()}:{line} arranges into a band invented from a bare extent "
                f"(`{spelling}`) with no reference to the control's own geometry. §B.6 rule 9 "
                f"requires the band to be the room the caller supplied, or the layout can never "
                f"see a shortfall and every child `min` is unobservable"
            )
    return findings


def check_rule_5_paired_links() -> list[str]:
    """[5] A host that puts a child on the tree must write **both** sides of the link.

    # The defect this catches

    `BaseWidget::add_child` appends to the parent's list and, by its own documentation, deliberately
    leaves the child's own parent link alone; `set_parent` symmetrically does not walk back. That is
    the right design for `BaseWidget` — it holds only an `ObjectId` and owns no registry, so it
    cannot reach the child. The consequence is that every *host* must remember both writes, and
    eight of them did not:

    `window::add_child`, `popup_window::set_content_widget`, `tab_widget::{add_tab, insert_tab}`,
    `tool_box::{add_item, insert_item}`, `mdi_area::add_sub_window`, `collapsible_pane`,
    `stacked_widget::{add_widget, insert_widget}`, `refresh_control`, plus `frame::set_widget` and
    `dock_widget::set_widget`.

    A child linked one-sided believes it has no parent, so no tree walk descending from the root —
    drawing, hit-testing, focus discovery, the accessibility submit — ever reaches it. The control
    is on screen and unreachable.

    # How the check stays honest

    The rule is not "never call `add_child`": a *leaf* calling it on itself (a test's fixture) or
    the runtime's own internal bookkeeping is fine, and `widget_trait.rs` forwards the trait method
    whose contract is the one-sided one. What is forbidden is a call that puts a **host's own**
    child on the tree without the child's half of the link, so the check is: a production call to
    `self.base.add_child(..)` in a file that is not the runtime, the trait default, or `base.rs`
    itself, must be `add_child_linked`.
    """
    findings: list[str] = []
    # Files where a bare `add_child` is the *implementation* of the one-sided contract rather than
    # a use of it.
    allowed = {
        "src/widget/base.rs",
        "src/widget/widget_trait.rs",
        "src/widget/runtime.rs",
    }
    pattern = re.compile(r"self\.base\.add_child\(")
    for path in sorted(pathlib.Path("src/widget").rglob("*.rs")):
        path_str = path.as_posix()
        if path_str in allowed:
            continue
        text = production_part(path.read_text())
        for match in pattern.finditer(text):
            line = text[: match.start()].count("\n") + 1
            findings.append(
                f"{path_str}:{line} puts a child on its own list with `self.base.add_child(..)`, "
                f"which does not write the child's parent link (§B.6 rule 5). Use "
                f"`self.base.add_child_linked(..)` so a tree walk from the root can reach the "
                f"child, or the child is on screen and unreachable"
            )
    return findings


def inject_rule_5() -> int:
    """Break §B.6 rule 5 and require the gate to notice."""
    path_str = "src/widget/container_widgets/stackedwidget.rs"
    path = pathlib.Path(path_str)
    original = path.read_text()
    broken = original.replace("self.base.add_child_linked(widget)", "self.base.add_child(widget)")
    if broken == original:
        print(f"❌ rule-5 injection point not found in {path_str}")
        return 1
    try:
        path.write_text(broken)
        found = check_rule_5_paired_links()
    finally:
        path.write_text(original)
    if found:
        print("✅ reverse injection: a one-sided child link is detected")
        return 0
    print("❌ reverse injection: a one-sided child link was NOT detected")
    return 1


def run() -> list[str]:
    findings: list[str] = []
    findings += check_builder_still_offers_the_channel()
    findings += check_one_hints_channel()
    findings += check_rule_5_paired_links()
    findings += check_rule_9_band_is_the_callers_room()
    checked = 0
    for name, path_str in COMPOSITES:
        path = pathlib.Path(path_str)
        if not path.exists():
            findings.append(f"{name}: {path_str} does not exist; the file moved and this gate is blind")
            continue
        text = production_part(path.read_text())
        if not is_composite(text):
            # Not a composite by the membership rule; skip rather than invent a finding.
            continue
        checked += 1
        findings += check_rule_1_factory_creation(path, text, name)
        findings += check_rule_2_layout_placement(path, text, name)
    if checked == 0:
        findings.append("no composite was recognised; the membership rule or the file list is wrong")
    print(f"composites checked: {checked} of {len(COMPOSITES)} listed")
    return findings


def inject() -> int:
    """Break §B.6 rule 2 in one composite and require the gate to notice."""
    name, path_str = COMPOSITES[0]
    path = pathlib.Path(path_str)
    original = path.read_text()
    # Replace the layout call with hand-computed placement, which is precisely the defect.
    broken = original.replace(
        INJECTION_TARGET,
        "placed.push(Rect::new(band.x + 8, band.y, 10, 10));\n"
        "        placed.push(Rect::new(band.x + 26, band.y, 10, 10));",
    )
    if broken == original:
        print(f"❌ injection point not found in {path_str}; this gate cannot prove it reads the file")
        return 1
    try:
        path.write_text(broken)
        found = check_rule_2_layout_placement(path, production_part(broken), name)
    finally:
        path.write_text(original)
    if found:
        print(f"✅ reverse injection: removing the layout call from {name} is detected")
        return 0
    print(f"❌ reverse injection: removing the layout call from {name} was NOT detected")
    return 1


def inject_rule_9() -> int:
    """Break §B.6 rule 9 (the band is the caller's room) and require the gate to notice.

    The injection restores exactly the shape the rule was written for — `tab_view`'s band
    derived from the tabs' own measured widths — so the check is proven against the defect that
    motivated it rather than against a synthetic string.
    """
    path_str = "src/widget/nav_widgets/tab_view.rs"
    path = pathlib.Path(path_str)
    original = path.read_text()
    broken = original.replace(
        "            let run = strip;",
        "            let run = Rect::new(0, 0, widths_total as u32, height);",
        1,
    )
    if broken == original:
        print(f"❌ rule-9 injection point not found in {path_str}")
        return 1
    try:
        path.write_text(broken)
        found = check_rule_9_band_is_the_callers_room()
    finally:
        path.write_text(original)
    if found:
        print("✅ reverse injection: a band invented from a bare extent is detected")
        return 0
    print("❌ reverse injection: a band invented from a bare extent was NOT detected")
    return 1


def inject_rule_1() -> int:
    """Break §B.6 rule 1 and require the gate to notice."""
    name, path_str = COMPOSITES[0]
    path = pathlib.Path(path_str)
    original = path.read_text()
    broken = original.replace(
        'let factory = WidgetFactory::new_with_defaults();',
        'let _unused_factory = 0;',
        1,
    )
    if broken == original:
        print(f"❌ rule-1 injection point not found in {path_str}")
        return 1
    try:
        path.write_text(broken)
        found = check_rule_1_factory_creation(path, production_part(broken), name)
    finally:
        path.write_text(original)
    if found:
        print(f"✅ reverse injection: removing the factory from {name} is detected")
        return 0
    print(f"❌ reverse injection: removing the factory from {name} was NOT detected")
    return 1


def main() -> int:
    if "--inject" in sys.argv:
        return inject() | inject_rule_1() | inject_rule_5() | inject_rule_9()

    findings = run()
    print()
    if findings:
        print(f"❌ a composite does not assemble its children the way §B.6 requires ({len(findings)}):")
        for finding in findings:
            print(f"   {finding}")
        return 1
    print("✅ composite assembly: every composite creates its children through the factory and")
    print("   places them with a layout")
    return 0


if __name__ == "__main__":
    sys.exit(main())
