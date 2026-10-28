# Widget Families in Detail

This chapter is the per-widget reference for the controls the crate ships. The [Widget System](widget-system.md) chapter explains the machinery every control shares; the [API Reference](api-reference.md) lists the modules and types; this chapter answers the question a host actually asks — *which control do I mount for this job, what can I set on it, what does it emit, and how do I call it?*

The chapter is organised by widget family — the module directories under `src/widget/` — and within a family by `WidgetKind`. Every kind declared in `src/widget/kind.rs` appears somewhere in these pages, including the kinds that only exist as aliases or as children of another control. The authoritative list of names is the capability registry (`WidgetFactory::capabilities()`); the authoritative list of kinds is the enum itself.

## How to read this chapter

Each control entry has the same shape:

* **Heading** — the `WidgetKind` variant, followed in parentheses by the canonical factory name. A kind can have several registered names (`line_edit`, `password_edit`, `phone_input` and `email_input` all report `WidgetKind::LineEdit`); each name gets its own entry because each has its own constructor and role.

* **Properties** — the property names, their `PropertyValueKind` and, for enumerated properties, the accepted tokens. Read-only properties are marked. Every schema also carries the four shared properties documented in the next section. The constant named in the entry (`BUTTON_PROPERTIES`, …) is the schema the control publishes, so a name can be checked against `src/widget/capability/`.

* **Events** — the names `WidgetFactory::connect_event` accepts, with the payload kind and shape (`Scalar`, `Tuple2`, `List`, optional, mixed). An event with no payload is a plain notification. The names and payloads come from the generated `event_payloads.rs` table, which `tools/check_event_payload_types.py` keeps aligned with the real signals.

* **Commands** — the names `WidgetFactory::invoke_command` accepts for this control.

* **Rust API** — the registered constructor, plus the two factory entry points a host normally uses. Constructors have the signature `fn(Rect, &str) -> Box<dyn Widget>`.

* **Aliases** — additional factory spellings, when the capability row publishes any.

Reading and writing a property by name, invoking a command and subscribing to an event all go through the same registry, so one entry point serves every control:

```rust
use rust_widgets::core::Rect;
use rust_widgets::signal::CustomSignalHub;
use rust_widgets::widget::capability::WidgetFactory;

let factory = WidgetFactory::shared();
let mut button = factory
    .create("button", Rect::new(10, 10, 120, 32), "Save")
    .expect("the registry knows `button`");

factory
    .write_property("button", &mut *button, "text", "Save as")
    .expect("`text` is a writable String property");
let current = factory
    .read_property("button", &*button, "text")
    .expect("`text` is readable");

let hub = CustomSignalHub::new();
factory
    .connect_event("button", "clicked", &hub, || { /* handle it */ })
    .expect("`button` publishes `clicked`");

factory
    .invoke_command("button", &mut *button, "click")
    .expect("`click` is a published command");
```

## The four shared properties

Every property schema in `src/widget/capability/` ends with the same four entries. They are implemented by the base layer rather than by each control, so they behave identically everywhere:

| Property | Type | Meaning |
|---|---|---|
| `enabled` | Bool | Whether the control accepts input. A disabled control still paints and still reports its model. |
| `visible` | Bool | Whether the control is painted and participates in hit testing. |
| `tooltip` | String | The hint shown on hover. The `TooltipManager` companion can also drive tooltips for a whole subtree. |
| `geometry` | String | The control's rectangle. The schema declares it readable and writable as `false`, so it is *not* reachable through `read_property`/`write_property`; placement belongs to the layout and to `Widget::set_geometry`. |

Several controls also expose a string spelling of an enum through a property whose accepted tokens are listed in the entry. Those tokens are the same spellings the control's setter parses, so a token read from the schema is always a token that can be written back. A property marked *read-only* can be read but a write returns `ReadOnlyProperty`; a property absent from an entry is not published at all and a write returns `UnknownProperty`.

## Families at a glance

| Family | Source directory | Controls | What it is for |
|---|---|---|---|
| `base` | `src/widget/base_widgets/` plus `src/widget/window.rs` | 8 | Window shells and the always-available controls |
| `overlay` | `src/widget/overlay_widgets/` | 5 | Floating actions, notices and lifecycle overlays |
| `nav` | `src/widget/nav_widgets/` | 7 | Top-level navigation and page containers |
| `input` | `src/widget/input_widgets/` | 29 | Text, choice and numeric entry |
| `display` | `src/widget/display_widgets/` | 27 | Read-only indicators and small display marks |
| `container` | `src/widget/container_widgets/` | 27 | Placement: scrolling, paging, splitting and layout boxes |
| `view` | `src/widget/view_widgets/` | 15 | Model-backed lists, trees, tables and inspectors |
| `chart` | `src/widget/chart_widgets/` | 5 | Line, bar, pie, spark and gauge visualization |
| `menu_toolbar` | `src/widget/menu_toolbar/` | 10 | Menus, toolbars, status bars and command items |
| `dialog` | `src/widget/dialog/` | 17 | Modal decisions and transient panels |
| `special` | `src/widget/special_widgets/` | 39 | Whole application surfaces and their companions |
| `code_editor` | `src/widget/special_widgets/code_editor/` and `src/widget/special_widgets/markdown_editor.rs` | 5 | Structured-text editors and their navigation chrome |
| `finance` | `src/widget/chart_widgets/finance/` | 6 | Market data: candles, depth, ladder and indicators |
| `advanced` | `src/widget/advanced_widgets/` | 10 | Date/time, dial, tabs, ribbon, pie menu and shortcuts |
| `misc` | `src/widget/misc_widgets/` | 9 | Utility and mobile controls |
| `media` | `src/widget/media_widgets/` | 7 | Playback, animation and camera preview |
| `web` | `src/widget/web_widgets/` | 1 | Embedded web content |
| `cupertino` | `src/widget/cupertino/` | 8 | iOS-styled and Material adaptive renditions |
| `macos` | `src/widget/macos/` | 5 | Native-shaped macOS chrome |

## 1. Base controls and window shells

The base family is the always-available core: the window and its grouping shells, and the four controls almost every form is built from. These are the only kinds a stripped profile keeps unconditionally, so a host that must run on a minimal build should build its screens from this family first.

Choose a `Button` for a single committed action, `CheckBox` or `ToggleButton` for an independent on/off setting, `RadioButton` for one choice inside a mutually exclusive group, and `Label` for text the user reads but does not edit. Reach for `Frame`, `GroupBox` and `Panel` to give a group of controls a visible boundary. Do not use a `Label` as an editable field, a `Button` as a navigation container, or a `Frame` as a layout engine — `Frame` draws a border and does not arrange children.

| Family | Source directory | Controls |
|---|---|---|
| `base` | `src/widget/base_widgets/` plus `src/widget/window.rs` | 8 |

### Window (`window`)

The top-level window: a title, a title bar with a close affordance, and the surface every other control is placed on. Use it once per top-level surface; use a `Dialog` or `PopupWindow` for a secondary or transient window, because those carry their own modality and lifecycle.

**Properties.** 4 published beyond the four shared: `title`: String · `title_bar_height`: UInt · `close_button_size`: UInt · `button_spacing`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `closed`

**Commands.** `set_title`, `close`

**Rust API.** `create_window(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("window", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `main_window`

### GroupBox (`panel`)

`Panel` is the `GroupBox` shell under its second factory name: a titled frame that groups related controls. In a full build `create_panel` mounts the `panel` name and the control reports `WidgetKind::GroupBox`; use it exactly like a `GroupBox`, and do not look for a `Panel`-only capability row — there is none.

**Properties.** 4 published beyond the four shared: `title`: String · `alignment`: Enum (`left` | `centre` | `right` | `top` | `bottom`) · `checkable`: Bool · `checked`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `toggled`: Bool

**Commands.** `set_title`, `set_checkable`, `set_checked`, `toggle`

**Rust API.** `create_panel(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("panel", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Frame (`frame`)

A frame draws a border, a 3D shadow or a separator line around one child. Use it for visual structure around content the layout already places; do not use it to arrange children or to clip them — it paints a shell and nothing else.

**Properties.** 7 published beyond the four shared: `frame_shape`: Enum (`no_frame` | `box` | `panel` | `styled_panel` | `hline` | `vline` | `win_panel`) · `frame_shadow`: Enum (`plain` | `raised` | `sunken`) · `line_width`: Float · `mid_line_width`: Float · `frame_width`: Float · `contents_margins`: String · `theme_bevel`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** `set_frame_shape`, `set_frame_shadow`, `set_line_width`

**Rust API.** `create_frame(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("frame", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `frame_widget`

### Button (`button`)

A push button fires `clicked` when activated and reports press, release and cancel separately. Use it for one committed action per button; put a menu behind a button with `menu_open` and a `MenuPanel` companion rather than building your own popup, and do not use it as a navigation container.

**Properties.** 17 published beyond the four shared: `text`: String · `pressed`: Bool · `default`: Bool · `icon_source`: IconSpec · `icon_size`: UInt · `shortcut`: String · `auto_default`: Bool · `auto_exclusive`: Bool · `flat`: Bool · `elide`: Enum (`none` | `left` | `middle` | `right`) · `drag_enabled`: Bool · `accepts_drops`: Bool · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt · `menu_open`: Bool · `menu_item_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `canceled` · `clicked` · `item_triggered`: UInt · `pressed` · `released` · `state_changed`: String

**Commands.** `click`

**Rust API.** `create_button(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("button", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `pushbutton`, `btn`

### Label (`label`)

A non-editable text surface with alignment, wrapping, eliding, indentation and optional rich text with links. Use it for captions, help text and inline links; use a `LineEdit`, `TextArea` or `RichEdit` when the user must change the text.

**Properties.** 10 published beyond the four shared: `text`: String · `alignment`: Enum (`left` | `centre` | `right` | `top` | `bottom`) · `word_wrap`: Bool · `elide`: Enum (`none` | `left` | `middle` | `right`) · `margin`: UInt · `scaled_contents`: Bool · `indent`: UInt · `text_format`: Enum (`plain` | `rich`) · `open_external_links`: Bool · `link_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `link_activated`: String · `link_hovered`: String

**Commands.** `set_text`, `set_alignment`

**Rust API.** `create_label(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("label", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `text_label`

### CheckBox (`check_box`)

A labelled box with an independent checked state and an optional third `partially_checked` state. Use it for settings that stand alone; use `RadioButton` when exactly one of several must be selected, and remember that `auto_exclusive` makes siblings exclusive without a radio group.

**Properties.** 8 published beyond the four shared: `text`: String · `state`: Enum (`unchecked` | `partially_checked` | `checked`) · `checked`: Bool · `tristate_enabled`: Bool · `auto_exclusive`: Bool · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `state_changed`: String · `toggled`: Bool

**Commands.** `set_checked`, `toggle`, `set_state`

**Rust API.** `create_check_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("check_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

### RadioButton (`radio_button`)

A round choice control that is mutually exclusive with the other radio buttons sharing its `group_id`. Use it for a small, visible set of alternatives; use a `ComboBox` when the list is long, and a `CheckBox` when the choices are independent.

**Properties.** 7 published beyond the four shared: `text`: String · `checked`: Bool · `group_id`: String · `auto_exclusive`: Bool · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `checked_changed`: Bool · `selected`

**Commands.** `set_checked`, `set_group_id`

**Rust API.** `create_radio_button(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("radio_button", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ToggleButton (`toggle_button`)

A button that latches between checked and unchecked, optionally with a popup arrow (`arrow_type`/`popup_mode`) and a menu. Use it for a mode that stays on, or for a tool button that owns a menu; use a plain `Button` for a one-shot action and a `Switch` for a settings-style on/off row.

**Properties.** 14 published beyond the four shared: `text`: String · `checked`: Bool · `auto_exclusive`: Bool · `auto_raise`: Bool · `group_id`: String · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt · `icon_source`: IconSpec · `arrow_type`: Enum (`none` | `down` | `up` | `left` | `right`) · `popup_mode`: Enum (`instant` | `menu_button` | `delayed`) · `menu_open`: Bool · `menu_item_count`: UInt (read-only) · `state`: Enum (`normal` | `hover` | `pressed` | `checked` | `disabled`) (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `checked_changed`: Bool · `item_triggered`: UInt · `pressed` · `released` · `state_changed`: String · `toggled`: Bool

**Commands.** `set_checked`, `toggle`, `set_text`

**Rust API.** `create_toggle_button(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("toggle_button", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `toggle`

## 2. Overlay and floating controls

Overlay controls sit above page content instead of taking a slot in the reading order. They cover the primary floating action, persistent notices, gesture-driven dismissal, pull-to-refresh and the startup screen.

Choose `FAB` for the one primary action of a page, `Banner` for a notice the user must acknowledge, `SwipeToDismiss` for list rows removed by a deliberate swipe, `RefreshControl` for pull-to-refresh over a scrollable body, and `SplashScreen` while the application initialises. Do not use an overlay for a decision that must block the rest of the UI — that is a dialog — and do not use `Banner` where a transient `Toast` or `Snackbar` is meant.

| Family | Source directory | Controls |
|---|---|---|
| `overlay` | `src/widget/overlay_widgets/` | 5 |

### Banner (`banner`)

A full-width, persistent notice with a severity, up to several actions and a user-dismissed state. Use it for a condition the user must see and acknowledge; use a `Snackbar` or `Toast` for something transient, and a `MessageBox` when the user must answer before continuing.

**Properties.** 5 published beyond the four shared: `text`: String · `severity`: Enum (`info` | `success` | `warning` | `error`) · `dismissible`: Bool · `dismissed`: Bool (read-only) · `action_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `action_clicked`: String · `dismissed`

**Commands.** `dismiss`, `show`, `set_actions`, `activate_action`

**Rust API.** `create_banner(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("banner", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `notice`

### FAB (`fab`)

A round, elevated primary-action button that floats over content. Use one per page for the main action; if the action expands into several, pair it with the `SpeedDial` companion rather than stacking FABs, and do not use it for destructive or rarely used commands.

**Properties.** 2 published beyond the four shared: `icon`: String · `icon_source`: IconSpec — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `clicked`

**Commands.** `set_icon`

**Rust API.** `create_fab(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("fab", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `floating_action_button`, `action_button`

### RefreshControl (`refresh_control`)

A pull-to-refresh surface: the user pulls the content down past a threshold and the control emits `refresh_triggered`; the host sets `refreshing` while the work runs. Use it around a scrollable body on touch devices; on desktop prefer an explicit refresh command.

**Properties.** 1 published beyond the four shared: `refreshing`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `refresh_triggered`

**Commands.** `set_refreshing`

**Rust API.** `create_refresh_control(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("refresh_control", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `pull_to_refresh`

### SplashScreen (`splash_screen`)

The startup screen: a logo/title, an optional subtitle and progress, dismissed by the program when initialisation finishes and not by the user. Use it only while the application is starting; use a `ProgressDialog` for work started by the user after the window is already visible.

**Properties.** 4 published beyond the four shared: `title`: String · `subtitle`: String · `progress`: Float · `skippable`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `fade_out_finished` · `finished`: String · `skipped`: String

**Commands.** `finish`, `set_progress`, `set_title`

**Rust API.** `create_splash_screen(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("splash_screen", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `splash`

### SwipeToDismiss (`swipe_to_dismiss`)

A gesture container that reveals an action background as its child is swiped and emits `dismissed` once the swipe passes the threshold. Use it for list rows whose primary removal gesture is a swipe; provide a visible alternative action for keyboard and desktop users, and pair it with `SwipeActions` when the background carries more than one action.

**Properties.** 1 published beyond the four shared: `is_dismissed`: Bool (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `dismissed`

**Commands.** No commands are published.

**Rust API.** `create_swipe_to_dismiss(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("swipe_to_dismiss", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `swipe_dismiss`

## 3. Navigation and page containers

Navigation controls express *where the user is* and *where they can go*: a top app bar, a bottom tab bar, a slide-out drawer, a push/pop page stack, a paged tab view, pagination and a scaffold that combines them.

Choose `AppBar` for a title plus leading/trailing actions, `BottomNavigationBar` for three to five top-level destinations on a phone, `NavigationDrawer` for a longer destination list, `NavigationStack` for a drill-down flow, `TabView` for peer pages, and `Pagination` for numbered pages owned by another control. Do not use navigation controls to host arbitrary content — `AdaptiveScaffold` composes the family, while `TabWidget` in the container family owns pages. Do not use `Pagination` as the page container: it is only the index.

| Family | Source directory | Controls |
|---|---|---|
| `nav` | `src/widget/nav_widgets/` | 7 |

### AdaptiveScaffold (`adaptive_scaffold`)

A scaffold that composes an `AppBar`, a content area and an optional bottom navigation bar into one adaptive frame. Use it as the top-level page shell on tablet and mobile; on desktop, place the navigation chrome explicitly so the window's own menu bar remains the primary command surface.

**Properties.** 5 published beyond the four shared: `title`: String · `badge_style`: Enum (`text` | `dot`) · `drawer_animated`: Bool · `scrim_opacity`: Float · `page_transition`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `nav_selected`: UInt

**Commands.** `set_title`

**Rust API.** `create_adaptive_scaffold(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("adaptive_scaffold", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `scaffold`

### AppBar (`app_bar`)

A top bar with a title, a leading back affordance and trailing actions; the two text affordances follow reading direction (`direction`), not fixed sides. Use it for a page title and one or two actions; move a longer action set into an overflow menu, and use a `ToolBar` on desktop.

**Properties.** 2 published beyond the four shared: `title`: String · `direction`: Enum (`ltr` | `rtl`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `action_index_pressed`: UInt · `action_overflow_pressed` · `action_pressed` · `back_pressed`

**Commands.** `set_title`

**Rust API.** `create_app_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("app_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `top_app_bar`

### BottomNavigationBar (`bottom_navigation_bar`)

A bottom row of three to five top-level destinations with selection state. Use it for phone-width top-level navigation; use a `NavigationDrawer` when there are more destinations, and a `TabView` when the pages are peers inside one screen rather than app-level destinations.

**Properties.** 2 published beyond the four shared: `item_count`: UInt (read-only) · `selected_index`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `current_changed`: UInt

**Commands.** `set_selected_index`

**Rust API.** `create_bottom_navigation_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("bottom_navigation_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `bottom_nav`, `bottomnav`

### NavigationDrawer (`navigation_drawer`)

A slide-in side panel with sections, headers and a selected item, plus a scrim that closes it. Use it for a long destination list or account/settings entries; add `DrawerSectionHeader` companions to group rows, and do not leave it permanently open on a wide layout where a sidebar belongs in the layout instead.

**Properties.** 3 published beyond the four shared: `open`: Bool · `width`: Float · `section_separator_visible`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `closed` · `header_clicked` · `item_selected`: UInt · `opened`

**Commands.** `set_open`, `set_width`

**Rust API.** `create_navigation_drawer(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("navigation_drawer", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `nav_drawer`, `drawer`

### NavigationStack (`navigation_stack`)

A push/pop page stack: `push`, `pop` and `set_current_page`, with `navigation_changed` reporting the new top page. Use it for a drill-down flow where back must return to the previous page; use a `StackedWidget` when the program, not the user, chooses the page and no history is implied.

**Properties.** 3 published beyond the four shared: `page_count`: UInt (read-only) · `current_page`: UInt · `back_visible`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `action_index_pressed`: UInt · `navigation_changed`: String

**Commands.** `push`, `pop`, `set_current_page`

**Rust API.** `create_navigation_stack(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("navigation_stack", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `nav_stack`

### Pagination (`pagination`)

A numbered page index with page size, sibling count, optional nav buttons and wheel paging. It holds no content: the `Carousel`, table or list owns the pages, and `Pagination` reports which page the user asked for. Do not use it as the page container itself.

**Properties.** 8 published beyond the four shared: `total`: UInt · `page_size`: UInt · `page`: UInt · `page_count`: UInt (read-only) · `last_page`: UInt (read-only) · `sibling_count`: UInt · `show_nav_buttons`: Bool · `wheel_paging_enabled`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `clicked`: UInt · `page_changed`: UInt · `page_hover_changed`: UInt or null

**Commands.** `next_page`, `previous_page`, `set_page`, `set_total`

**Rust API.** `create_pagination(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("pagination", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `pager`, `page_numbers`

### TabView (`tab_view`)

A tab strip plus a content slot, with optional keyboard navigation and close/move requests reported as events. Use it for peer pages inside one screen; use a `TabWidget` when each page is an arbitrary child container with its own layout, and a `BottomNavigationBar` for app-level destinations.

**Properties.** 3 published beyond the four shared: `tab_count`: UInt (read-only) · `selected_index`: UInt · `keyboard_navigation_enabled`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `tab_changed`: UInt · `tab_close_requested`: UInt · `tab_moved`: UInt ×2

**Commands.** `set_selected_index`

**Rust API.** `create_tab_view(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tab_view", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `page_tab_view`

## 4. Text and value input

The input family turns keystrokes, selection and drags into a typed value. It ranges from the single-line field to masked entry, completion, multi-line editing, a virtual keyboard and numeric pickers.

Choose by the *shape of the value*: `LineEdit` for one line of free text, `TextArea` or `TextEdit` for paragraphs, `SpinBox` or `NumberPicker` for bounded numbers, `ComboBox`/`Dropdown` for a fixed set of choices, `AutoCompleteEdit` or `Mention` when suggestions come from data, `MaskedEdit` when the format is fixed, and `Cascader` for a path through a hierarchy. Do not use `LineEdit` for multi-line text, a `ComboBox` as an editable free-text field (that is `EditableComboBox`), or `ListBox` for a dataset large enough to need virtualization — `VirtualList` belongs to the view family.

| Family | Source directory | Controls |
|---|---|---|
| `input` | `src/widget/input_widgets/` | 29 |

### LineEdit (`line_edit`)

A single-line text field with selection, undo/redo availability, echo modes and validation decoration (`helper`, `error`, `counter`, `validation_state`). Use it for one line of free text; use `TextArea` for paragraphs, `MaskedEdit` for a fixed format and `SpinBox` for a numeric value.

**Properties.** 21 published beyond the four shared: `text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (read-only) · `over_limit`: Bool (read-only) · `valid`: Bool (read-only) · `validation_message`: String (read-only) · `validation_state`: String (read-only) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**Commands.** `clear`, `select_all`

**Rust API.** `create_line_edit(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("line_edit", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `text_input`, `input`

### LineEdit (`password_edit`)

The password role of `LineEdit`: `echo_mode` is `password` and `password_echo_on_edit` controls the brief reveal while typing. Use it for secrets; never log the value, and use `phone_input` or `email_input` only for their own value shapes.

**Properties.** 21 published beyond the four shared: `text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (read-only) · `over_limit`: Bool (read-only) · `valid`: Bool (read-only) · `validation_message`: String (read-only) · `validation_state`: String (read-only) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**Commands.** `clear`, `select_all`

**Rust API.** `create_password_edit(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("password_edit", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `password_input`, `password_line_edit`

### LineEdit (`phone_input`)

The telephone role of `LineEdit`: the constructor installs the phone validator, whitespace and non-phone characters are refused, and `valid` reports whether the value holds 7 to 15 digits. Use it for a phone number; it validates but does not format the number for display.

**Properties.** 21 published beyond the four shared: `text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (read-only) · `over_limit`: Bool (read-only) · `valid`: Bool (read-only) · `validation_message`: String (read-only) · `validation_state`: String (read-only) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**Commands.** `clear`, `select_all`

**Rust API.** `create_phone_input(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("phone_input", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `phone_edit`, `telephone_input`

### LineEdit (`email_input`)

The e-mail role of `LineEdit`: the constructor installs the e-mail validator, whitespace is refused, and `valid` reports whether the value is `local@domain.tld`. Use it for an address; it validates the shape, but you still decide what to do with an invalid value.

**Properties.** 21 published beyond the four shared: `text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (read-only) · `over_limit`: Bool (read-only) · `valid`: Bool (read-only) · `validation_message`: String (read-only) · `validation_state`: String (read-only) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**Commands.** `clear`, `select_all`

**Rust API.** `create_email_input(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("email_input", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `email_edit`, `email_field`

### ListBox (`list_box`)

A scrollable list of items with single, multi or extended selection and a current row. Use it for a modest, fully materialized list where each row is plain text; use `ListView` or `VirtualList` once rows carry models, delegates or need virtualization.

**Properties.** 9 published beyond the four shared: `item_count`: UInt (read-only) · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `current_row`: UInt · `item_height`: Float · `selected_count`: UInt (read-only) · `selected_indices`: String (read-only) · `hovered_row`: UInt (read-only) · `font_size`: Float (read-only) · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `drag_finished`: String · `item_activated`: UInt · `item_changed`: UInt · `item_selected`: UInt · `rename_requested`: UInt · `selection_changed`

**Commands.** `add_item`, `remove_item`, `clear`, `clear_selection`, `set_selection_mode`

**Rust API.** `create_list_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("list_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

### SpinBox (`spin_box`)

A numeric field with step buttons, a range, decimals and optional prefix/suffix and special value text. Use it for bounded numbers; use `NumberPicker` for a wheel-style picker, `Dial` for an angular control, and `LineEdit` when the value is not numeric.

**Properties.** 14 published beyond the four shared: `minimum`: Number · `maximum`: Number · `value`: Number · `single_step`: Number · `decimals`: UInt · `prefix`: String · `suffix`: String · `display_text`: String (read-only) · `value_text`: String (read-only) · `special_value_text`: String · `wrapping`: Bool · `keyboard_tracking`: Bool · `direction`: Enum (`ltr` | `rtl`) · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `editing_finished` · `validation_failed`: String · `value_changed`: Int

**Commands.** `set_range`, `set_value`, `step_up`, `step_down`

**Rust API.** `create_spin_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("spin_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ComboBox (`combo_box`)

An item field with a drop-down list, an optional editable text and a visible popup state. Use it for a fixed set of choices with one current value; use `EditableComboBox` when the user may type a value that is not in the list, and `DropdownMenu` when the choices form a menu of commands rather than a value.

**Properties.** 13 published beyond the four shared: `item_count`: UInt (read-only) · `insert_policy`: Enum (`no_insert` | `insert_at_top` | `insert_at_current` | `insert_at_bottom` | `insert_alphabetically`) · `size_adjust_policy`: Enum (`adjust_to_minimum_contents_length` | `adjust_to_contents` | `adjust_to_contents_on_first_show`) · `items`: StringList · `current_index`: UInt · `current_text`: String · `placeholder_text`: String · `minimum_contents_length`: UInt · `editable`: Bool · `max_visible_items`: UInt · `open`: Bool · `direction`: String · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `activated`: UInt · `current_index_changed`: UInt or null · `current_text_changed`: String · `highlighted`: UInt · `popup_visibility_changed`: Bool · `text_edited`: String

**Commands.** `set_items`, `set_current_index`, `clear`

**Rust API.** `create_combo_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("combo_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Dropdown (`dropdown`)

A standalone drop-down selector with a placeholder, an expanded state and a visible item count. Use it when the field is only a chooser and carries no editable text; use `ComboBox` when the widget also owns a text value, and `MenuButton` when the popup is a command menu.

**Properties.** 8 published beyond the four shared: `text`: String (read-only) · `placeholder`: String · `selected_index`: UInt · `item_count`: UInt (read-only) · `expanded`: Bool · `max_visible`: UInt · `scroll_offset`: UInt · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `activated`: UInt · `changed` · `highlighted`: UInt

**Commands.** `set_items`, `set_selected_index`, `set_expanded`, `toggle`

**Rust API.** `create_dropdown(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("dropdown", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `dropdown_widget`, `combo`

### TextArea (`text_area`)

A multi-line plain-text input with soft wrap, a content margin, a block limit and horizontal scrolling. Use it for paragraphs and free-form notes; use `TextEdit` when you want the framed editor control with line wrap, and `CodeEditor` for structured source.

**Properties.** 8 published beyond the four shared: `text`: String · `placeholder`: String · `read_only`: Bool · `max_length`: UInt · `soft_wrap`: Bool · `content_margin`: UInt · `max_block_count`: UInt · `scroll_x`: Int (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `changed` · `context_menu_requested`: String · `cursor_position_changed`: UInt · `redo_available`: Bool · `selection_changed` · `undo_available`: Bool

**Commands.** `set_text`, `set_placeholder`, `set_read_only`, `insert`, `delete_char`

**Rust API.** `create_textarea(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("text_area", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `multiline_edit`

### TextEdit (`text_edit`)

A framed multi-line editor with selectable text, a cursor position and a line-wrap switch. Use it for a general-purpose multi-line field that needs the editor chrome; use `RichEdit` when formatting attributes are required, and `TerminalView` for append-only output with a prompt.

**Properties.** 6 published beyond the four shared: `text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `line_wrap`: Bool · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `cursor_position_changed`: UInt · `selection_changed` · `text_changed`: String

**Commands.** `set_text`, `set_placeholder_text`, `set_max_length`, `set_read_only`, `set_line_wrap`

**Rust API.** `create_text_edit(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("text_edit", geometry, text)` and `WidgetFactory::shared()` for the registry.

### CommandLink (`command_link`)

A button with a caption, an explanatory description and an optional shield marker — the Windows-style command link. Use it for a small set of consequential choices in a dialog; use a plain `Button` when a one-line caption is enough.

**Properties.** 3 published beyond the four shared: `text`: String · `description`: String · `shield`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `clicked` · `hovered`: Bool

**Commands.** `set_text`, `set_description`, `set_enabled`, `click`

**Rust API.** `create_command_link(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("command_link", geometry, text)` and `WidgetFactory::shared()` for the registry.

### FontComboBox (`font_combo_box`)

A combo box populated with installed typefaces, rendered in each font, with filters and a sample text. Use it as the font chooser inside a `FontDialog` or a settings form; use `FontPreview` to show a larger specimen of the current choice.

**Properties.** 9 published beyond the four shared: `current_font_family`: String (read-only) · `item_count`: Int (read-only) · `current_index`: Int · `editable`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `max_visible_items`: Int · `font_filters`: String · `writing_system_filter`: String · `sample_text`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `activated`: Int · `current_font_changed`: String · `current_index_changed`: Int · `popup_hidden` · `popup_shown` · `text_edited`: String

**Commands.** `set_current_index`, `set_editable`, `set_max_visible_items`, `show_popup`, `hide_popup`

**Rust API.** `create_font_combo_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("font_combo_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Keyboard (`keyboard`)

An on-screen virtual keyboard with a qwerty or numeric layout, a shift state and key events; `text_committed` carries the characters to insert. Use it on touch devices without a hardware keyboard; keep a hardware path working, and do not present it on desktop where it steals screen space.

**Properties.** 3 published beyond the four shared: `layout`: Enum (`qwerty` | `numeric`) · `lowercase`: Bool · `press_animation`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `backspace_pressed` · `enter_pressed` · `key_pressed`: UInt ×2 · `space_pressed` · `text_committed`: String

**Commands.** `set_layout`, `set_lowercase`, `toggle_shift`

**Rust API.** `create_keyboard(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("keyboard", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `keyboard_widget`, `virtual_keyboard`

### Mention (`mention`)

A text field that completes the token before the caret from a candidate list, allowing several mentions in one field. Use it for `@`-style mentions in a message or comment; use `AutoCompleteEdit` when the whole field is completed against one suggestion.

**Properties.** 9 published beyond the four shared: `text`: String · `trigger`: String · `candidate_count`: UInt (read-only) · `visible_candidate_count`: UInt (read-only) · `mention_count`: UInt (read-only) · `popup_open`: Bool (read-only) · `caret`: UInt (read-only) · `max_visible`: UInt · `row_height`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `mention_inserted`: String · `popup_toggled`: Bool · `text_changed`: String

**Commands.** `set_candidates`, `set_trigger`, `complete`, `open_popup`

**Rust API.** `create_mention(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("mention", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `mention_edit`, `at_mention`

### Cascader (`cascader`)

A multi-level chooser that walks a path down a tree of options and reports the selected path. Use it for a hierarchical taxonomy where every level matters; use a `ComboBox` for a flat list and a `TreeView` when the hierarchy itself must stay visible and browsable.

**Properties.** 8 published beyond the four shared: `value`: String (read-only) · `placeholder`: String · `depth`: UInt (read-only) · `expanded`: Bool · `separator`: String · `filterable`: Bool · `filter`: String (read-only) · `max_visible`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `selection_changed`: list of UInt

**Commands.** `set_options`, `set_selected_path`, `expand`, `collapse`

**Rust API.** `create_cascader(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("cascader", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `cascade`, `multi_level_select`

### SearchBox (`search_box`)

A single-line search field with a leading search affordance and a trailing clear button; `search_submitted` carries the query. Use it above a list or table for a quick filter; use `SearchBar` when you also need a cancel action and platform search styling.

**Properties.** 3 published beyond the four shared: `text`: String · `placeholder`: String · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `search_submitted`: String · `text_changed`: String

**Commands.** `set_text`, `set_placeholder`, `clear`

**Rust API.** `create_search_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("search_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `search`

### TagInput (`tag_input`)

A text field that turns Enter or a comma into a removable tag, with a multi-line mode. Use it for a free-form set of short labels; use the `Chip` row control when the set is fixed and only selection changes, and `Mention` when the tokens refer to people rather than arbitrary labels.

**Properties.** 4 published beyond the four shared: `tags`: String (read-only) · `multiline`: Bool · `placeholder`: String · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `tags_changed`: list of String

**Commands.** `add_tag`, `remove_tag`, `set_placeholder`

**Rust API.** `create_tag_input(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tag_input", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `tag_editor`, `chip_input`

### ImePreedit (`ime_preedit`)

An overlay that draws the input method's pre-edit text and caret at a position supplied by the host. Use it only when implementing IME composition over a custom text surface; controls that already accept IME input (`LineEdit`, `TextArea`, …) place their pre-edit themselves.

**Properties.** 2 published beyond the four shared: `text`: String · `cursor_position`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** `set_text`, `set_cursor_position`

**Rust API.** `create_ime_preedit(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("ime_preedit", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `preedit`

### InplaceEditor (`inplace_editor`)

A small editor that starts and finishes editing in place, used by table and tree cells. Use it for cell editing where the value must be committed with Enter and abandoned with Escape; use `LineEdit` when the field is a permanent part of the form.

**Properties.** 3 published beyond the four shared: `text`: String · `editing`: Bool · `edit_triggers`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `edit_accepted`: String · `edit_canceled`

**Commands.** `set_text`, `start_editing`, `finish_editing`

**Rust API.** `create_inplace_editor(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("inplace_editor", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `inline_editor`

### MaskedEdit (`masked_edit`)

A formatted text field driven by a mask, rejecting input that does not fit and reporting `input_rejected` with the offending position. Use it for phone numbers, dates, postal codes and identifiers with a fixed shape; use `LineEdit` plus validation when the format is advisory rather than mandatory.

**Properties.** 3 published beyond the four shared: `text`: String · `mask`: String · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `editing_finished` · `input_rejected`: UInt · `return_pressed` · `text_changed`: String

**Commands.** `set_text`, `set_mask`

**Rust API.** `create_masked_edit(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("masked_edit", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `masked_input`

### AutoCompleteEdit (`auto_complete_edit`)

A text field that matches its whole value against a candidate list and offers a drop-down, with match mode, case sensitivity, inline completion and a close policy. Use it for a single value chosen from suggestions; use `Mention` for token completion and `ComboBox` when the list is closed.

**Properties.** 15 published beyond the four shared: `text`: String · `placeholder`: String · `suggestion_count`: UInt (read-only) · `selected_index`: UInt · `selected_suggestion`: String (read-only) · `dropdown_visible`: Bool (read-only) · `max_visible`: UInt · `alignment`: Enum (`left` | `centre` | `right`) · `can_undo`: Bool (read-only) · `can_redo`: Bool (read-only) · `match_mode`: Enum (`contains` | `starts_with` | `ends_with` | `fixed_string` | `regexp`) · `case_sensitive`: Bool · `inline_completion`: Bool · `close_policy`: Enum (`always` | `never` | `on_commit`) · `completion_prefix`: String (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `suggestion_selected`: String · `text_changed`: String

**Commands.** `set_text`

**Rust API.** `create_auto_complete_edit(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("auto_complete_edit", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `autocomplete`

### MultiSelectComboBox (`multi_select_combo_box`)

A combo box whose list keeps several selections and reports them as a selected set. Use it for a compact multi-choice field; use `CheckListBox` when the choices need their own always-visible rows, and `TagInput` when the user may also enter values that are not offered.

**Properties.** 5 published beyond the four shared: `selected_count`: UInt (read-only) · `selected_items`: String (read-only) · `placeholder`: String · `expanded`: Bool · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `selection_changed`: list of UInt

**Commands.** `set_expanded`

**Rust API.** `create_multi_select_combo_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("multi_select_combo_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `multi_combo`

### RangeSlider (`range_slider`)

A dual-handle slider that selects a lower and an upper bound on one track and reports both as `range_changed`. Use it for a coarse range where both ends move together visually; use two `SpinBox` fields when the bounds must be typed exactly, and a `DateRangePicker` for dates.

**Properties.** 11 published beyond the four shared: `min_value`: Float · `step`: Float · `min_range`: Float · `tick_interval`: Float · `tick_labels`: Bool · `value_hint_enabled`: Bool · `max_value`: Float · `lower`: Float · `upper`: Float · `orientation`: Enum (`horizontal` | `vertical`) · `direction`: Enum (`ltr` | `rtl`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `range_changed`: Float ×2

**Commands.** `set_lower`, `set_upper`, `set_range`

**Rust API.** `create_range_slider(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("range_slider", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `dual_slider`

### EditableComboBox (`editable_combo_box`)

A combo box whose field can hold text that is not in the list; `item_selected` reports a pick and `text_changed` reports typing. Use it for a value that is usually chosen but may be entered; use `ComboBox` for a closed set and `AutoCompleteEdit` when suggestions must be matched as the user types.

**Properties.** 6 published beyond the four shared: `text`: String · `item_count`: UInt (read-only) · `cursor_position`: UInt · `max_visible_items`: UInt · `scroll_offset`: UInt · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `editing_finished` · `item_selected`: UInt · `return_pressed` · `text_changed`: String

**Commands.** `set_text`

**Rust API.** `create_editable_combo_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("editable_combo_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `editable_combo`

### SearchBar (`search_bar`)

A platform-styled search field with a cancel action, `search_submitted` and `text_changed`. Use it for a search-and-cancel flow on iOS-style screens; use `SearchBox` for the compact desktop field without a cancel affordance.

**Properties.** 3 published beyond the four shared: `text`: String · `placeholder`: String · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `canceled` · `search_submitted`: String · `text_changed`: String

**Commands.** `set_text`, `set_placeholder`

**Rust API.** `create_search_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("search_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `search_field`

### ShortcutEditor (`shortcut_editor`)

A searchable list of keyboard shortcuts grouped by category, with filtering, an empty state and undo/redo availability. Use it for the shortcut reference/editor panel; use `KeySequenceEdit` when the task is recording one new shortcut.

**Properties.** 8 published beyond the four shared: `filter_text`: String · `shortcut_count`: UInt (read-only) · `visible_count`: UInt (read-only) · `category_count`: UInt (read-only) · `categories`: String (read-only) · `empty`: Bool (read-only) · `can_undo`: Bool (read-only) · `can_redo`: Bool (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `shortcut_changed`: mixed (String parts)

**Commands.** `set_filter_text`

**Rust API.** `create_shortcut_editor(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("shortcut_editor", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `keyboard_shortcut_editor`

### OtpInput (`otp_input`)

A segmented code field: one box per character, automatically advancing, with paste, backspace and a completion signal. Use it for one-time codes and short PINs; use `MaskedEdit` for a longer formatted identifier and `LineEdit` for an ordinary password.

**Properties.** 8 published beyond the four shared: `value`: String · `length`: UInt · `masked`: Bool · `read_only`: Bool · `digits_only`: Bool · `separator`: String · `focused_index`: UInt (read-only) · `is_complete`: Bool (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `canceled` · `completed`: String · `submitted`: String · `value_changed`: String

**Commands.** `insert_char`, `backspace`, `paste`, `clear`

**Rust API.** `create_otp_input(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("otp_input", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `otp`

### NumberPicker (`number_picker`)

A vertically scrolling digit wheel that shows neighbouring values and changes by a drag or flick rather than per-step clicks. Use it for a bounded number chosen from a short range; use `SpinBox` when the value is typed or stepped exactly.

**Properties.** 12 published beyond the four shared: `value`: Int · `minimum`: Int · `maximum`: Int · `step`: Int · `wrap`: Bool · `suffix`: String · `prefix`: String · `decimals`: UInt · `special_value_text`: String · `row_count`: UInt (read-only) · `selected_row`: UInt (read-only) · `row_height`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `value_changed`: Int

**Commands.** `scroll_rows`, `set_value`, `set_range`, `set_step`

**Rust API.** `create_number_picker(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("number_picker", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `picker`

### RichEdit (`rich_edit`)

A multi-line editor with character and paragraph formatting attributes, read-only and wrap switches and cursor/selection events. Use it for formatted prose; use `MarkdownEditor` when the source is markdown, and `TextArea` when the text is plain.

**Properties.** 5 published beyond the four shared: `text`: String · `line_count`: UInt (read-only) · `read_only`: Bool · `line_wrap`: Bool · `placeholder_text`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `cursor_position_changed`: UInt · `read_only_changed`: Bool · `selection_changed`: (UInt, UInt) or null · `text_changed`: String

**Commands.** `set_text`

**Rust API.** `create_rich_edit(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("rich_edit", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `rich_text_editor`

## 5. Display and indicators

Display controls report state instead of accepting it: progress, values, icons, status marks, swatches and small decorative surfaces. Several are also the visual half of an input pair (for example `Slider` plus a value label).

Choose a display control when the user reads rather than edits: `ProgressBar` and `ProgressCircle` for completion, `Slider` and `ScrollBar` only when the value is directly manipulated, `Arc`, `Meter`, `LCDNumber` and `Roller` for instrument-like readouts, `Badge`, `Tag`, `Chip` and `Avatar` for compact status, `Divider` and `Line` for separation. Do not use a display control as the storage of record — set its value and also keep the model. Do not use `Slider` for a value that must be typed exactly; pair it with a `SpinBox`.

| Family | Source directory | Controls |
|---|---|---|
| `display` | `src/widget/display_widgets/` | 27 |

### Slider (`slider`)

A handle dragged along a groove to pick an integer within a range, with ticks, page steps and tracking. Use it for a coarse, directly manipulated value; use a `SpinBox` for exact entry, a `Dial` for an angular instrument, and `RangeSlider` for two bounds.

**Properties.** 10 published beyond the four shared: `minimum`: Int · `maximum`: Int · `value`: Int · `single_step`: Int · `page_step`: Int · `orientation`: Enum (`horizontal` | `vertical`) · `tick_position`: Enum (`none` | `above` | `below` | `both`) · `tick_interval`: Int · `tracking`: Bool · `slider_position`: Int — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `slider_moved`: Int · `slider_pressed` · `slider_released` · `value_changed`: Int

**Commands.** `set_range`, `set_value`, `set_slider_position`

**Rust API.** `create_slider(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("slider", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ProgressBar (`progress_bar`)

A bounded fill indicator with an optional text label, orientation and an indeterminate mode. Use it for a known fraction of a known total; use `ProgressCircle` for a compact circular rendition and `Spinner` when duration is unknown and only activity matters.

**Properties.** 9 published beyond the four shared: `minimum`: Int · `maximum`: Int · `value`: Int · `text_visible`: Bool · `orientation`: Enum (`horizontal` | `vertical`) · `inverted_appearance`: Bool · `indeterminate`: Bool · `direction`: Enum (`ltr` | `rtl`) · `progress`: Float (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `value_changed`: Int

**Commands.** `set_range`, `set_value`, `set_orientation`

**Rust API.** `create_progress_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("progress_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ScrollBar (`scroll_bar`)

A thumb on a rail that scrolls a viewport, with idle fading, page steps and a slider-size ratio. Use it beside a custom scrollable surface; use `ScrollArea` when the crate should own the viewport and place the bar for you.

**Properties.** 11 published beyond the four shared: `idle_delay_ms`: UInt · `idle_opacity`: Float · `minimum`: Int · `maximum`: Int · `value`: Int · `single_step`: Int · `page_step`: Int · `orientation`: Enum (`horizontal` | `vertical`) · `direction`: Enum (`ltr` | `rtl`) · `slider_size`: Float (read-only) · `slider_position`: Float (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `slider_moved`: Int · `slider_pressed` · `slider_released` · `value_changed`: Int

**Commands.** `set_range`, `set_value`, `set_steps`, `set_orientation`

**Rust API.** `create_scroll_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("scroll_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

### LCDNumber (`lcd_number`)

A seven-segment numeric readout with a configurable digit count, number base and segment style. Use it for instrument-style numbers (counters, clocks, measurements); use a `Label` for ordinary text and a `Meter` when the value also has a range context.

**Properties.** 7 published beyond the four shared: `value`: Float · `min_value`: Float · `max_value`: Float · `num_digits`: Int · `small_decimal_point`: Bool · `mode`: Enum (`hex` | `dec` | `oct` | `bin`) · `segment_style`: Enum (`outline` | `filled` | `flat`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `overflow` · `value_changed`: Float

**Commands.** `set_value`, `set_mode`, `set_segment_style`

**Rust API.** `create_lcd_number(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("lcd_number", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Arc (`arc`)

A circular progress or indicator ring with a sweep angle, thickness, optional rounded ends and a centred value. Use it for a compact single-value dial; use `ProgressCircle` for a Material-style progress ring and `Gauge` when a scale and needle are needed.

**Properties.** 10 published beyond the four shared: `value_format`: String · `value`: UInt · `minimum`: UInt · `maximum`: UInt · `thickness`: UInt · `sweep_angle`: UInt · `start_angle`: Float · `indeterminate`: Bool · `rounded`: Bool · `show_value`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `changed`

**Commands.** `set_value`, `set_range`

**Rust API.** `create_arc(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("arc", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `arc_widget`

### Spinner (`spinner`)

A rotating activity indicator with an arc or spokes style, speed, thickness and size ratio. Use it when a request is in flight and duration is unknown; use `ProgressBar` when a fraction is known, and stop the spinner when the operation ends so it never animates forever.

**Properties.** 7 published beyond the four shared: `active`: Bool · `track_color`: Color · `arc_color`: Color · `thickness`: UInt · `speed`: Float · `size_ratio`: Float · `style`: Enum (`arc` | `spokes`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `active_changed`: Bool

**Commands.** `set_active`, `set_speed`, `set_thickness`, `set_size_ratio`

**Rust API.** `create_spinner(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("spinner", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `spinner_widget`, `loading_spinner`

### Roller (`roller`)

A scroll-wheel style selector that shows the current option with its neighbours above and below; a drag or flick moves the selection. Use it for a short, ordered set of options on a touch screen; use `ComboBox` on desktop and `NumberPicker` when the values are numeric.

**Properties.** 4 published beyond the four shared: `options`: StringList · `selected_index`: UInt · `visible_count`: UInt · `item_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `changed`

**Commands.** `set_options`, `set_selected_index`, `set_visible_count`

**Rust API.** `create_roller(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("roller", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `roller_widget`, `scroll_wheel`

### Switch (`switch`)

An on/off track-and-thumb control with Material, Cupertino or macOS geometry. Use it for a setting that applies immediately; use `CheckBox` when the choice is part of a form submitted later, and `CupertinoSwitch` when the iOS rendition is required by the design.

**Properties.** 4 published beyond the four shared: `checked`: Bool · `thumb_color`: Color · `on_track_color`: Color · `switch_geometry`: Enum (`material` | `cupertino` | `macos`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `toggled`: Bool

**Commands.** `set_checked`, `toggle`

**Rust API.** `create_switch(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("switch", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `switch_widget`, `toggle_switch`

### Line (`line`)

A horizontal or vertical rule with a thickness and colour, used as a lightweight separator. Use it inside a custom layout that needs one rule; use `Divider` when you want the platform separator style and orientation property.

**Properties.** 3 published beyond the four shared: `orientation`: Enum (`horizontal` | `vertical`) · `thickness`: UInt · `color`: Color — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** `set_orientation`

**Rust API.** `create_line(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("line", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `line_widget`

### Meter (`meter`)

A gauge-like meter with a value against a range, ticks, an optional unit and threshold bands. Use it for a measurement with a meaningful scale; use `Gauge` for a full radial dial, `Arc` for a compact ring, and `ProgressBar` for plain completion.

**Properties.** 8 published beyond the four shared: `value`: UInt · `minimum`: UInt · `maximum`: UInt · `tick_count`: UInt · `show_tick_labels`: Bool · `unit`: String · `value_text`: String (read-only) · `threshold_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `changed`

**Commands.** `set_value`, `set_range`

**Rust API.** `create_meter(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("meter", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `meter_widget`

### EmojiPicker (`emoji_picker`)

A glyph chooser shell with categories, search, recents and a grid; the glyph table itself is supplied by the caller through `set_glyphs`, because the crate bundles no emoji data. Use it in a composer or reaction picker, and handle `glyph_chosen` to insert the character.

**Properties.** 7 published beyond the four shared: `recent_capacity`: UInt · `glyph_count`: UInt (read-only) · `visible_glyph_count`: UInt (read-only) · `tab_count`: UInt (read-only) · `active_tab`: UInt · `search`: String · `recent_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `glyph_chosen`: String · `glyph_hover_changed`: String

**Commands.** `set_glyphs`, `set_categories`, `set_search`, `choose`

**Rust API.** `create_emoji_picker(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("emoji_picker", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `emoji`

### MiniChart (`mini_chart`)

A compact line or bar chart with no axes, intended to sit inside a row, tile or card. Use it for a glanceable trend; use `Sparkline` for a line-only inline trend with a capacity and end marker, and the chart family when axes and interaction matter.

**Properties.** 1 published beyond the four shared: `chart_type`: Enum (`line` | `bar`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `point_hover_changed`: UInt or null

**Commands.** `set_chart_type`, `set_data`, `set_range`

**Rust API.** `create_mini_chart(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("mini_chart", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `mini_chart_widget`

### ImageView (`image_view`)

A widget that displays an `Image` with scaling, rotation, flips, alignment, margin and a zoom/pan offset. Use it for a single still image; use `ImageGallery` for a set with thumbnails, `AnimatedImage` for frame sequences, and pair it with `ZoomPanController` when zoom and region-of-interest state must be shared.

**Properties.** 6 published beyond the four shared: `scaled`: Bool · `rotation`: Enum (`0` | `90` | `180` | `270`) · `flip_horizontal`: Bool · `flip_vertical`: Bool · `zoom_level`: Float · `pan_offset`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `double_clicked` · `image_changed` · `load_failed`: String

**Commands.** `set_image`, `set_scaled`

**Rust API.** `create_image_view(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("image_view", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `image_viewer`

### MiniCanvas (`mini_canvas`)

A small immediate-mode drawing surface with fill/draw primitives and clear. Use it for a decorative drawing or a sparkline-like custom mark; use the full `Canvas` when layers, zoom and pointer routing are needed, and keep drawing out of `paint`-only controls.

**Properties.** The schema publishes no property of its own; every control carries the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `clicked` · `mouse_pressed` · `mouse_released`

**Commands.** `fill_rect`, `draw_rect`, `draw_line`, `fill_circle`, `clear`

**Rust API.** `create_mini_canvas(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("mini_canvas", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Badge (`badge`)

A small overlay marker carrying a count, a dot or a short label, anchored to another control. Use it for unread counts and status dots; it is an annotation, not a button, so put the clickable behaviour on the anchored control.

**Properties.** 4 published beyond the four shared: `text`: String · `count`: Int · `dot_mode`: Bool · `level`: Enum (`info` | `success` | `warning` | `error`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `count_changed`: UInt · `level_changed`: String

**Commands.** `set_text`, `set_count`

**Rust API.** `create_badge(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("badge", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `badge_widget`, `notification_badge`

### SkeletonLoader (`skeleton_loader`)

A grey placeholder block shown while real content loads, with rectangle, circle and text-line shapes. Use it to preserve layout and signal progress; do not leave it visible after content arrives, and use a `Spinner` when there is no shape to preview.

**Properties.** 3 published beyond the four shared: `active`: Bool · `text_line_rows`: UInt · `shape`: Enum (`rect` | `circle` | `text_line`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** `set_active`

**Rust API.** `create_skeleton_loader(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("skeleton_loader", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `skeleton`, `shimmer`

### Divider (`divider`)

A thin rule that separates adjacent sections, with a line style, orientation, thickness and colour. Use it between groups in a form or list; use `Line` when you need a free-standing rule in a custom drawing.

**Properties.** 4 published beyond the four shared: `line_style`: Enum (`plain` | `sunken` | `raised`) · `orientation`: String · `thickness`: Float · `color`: Color — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** `set_orientation`, `set_thickness`

**Rust API.** `create_divider(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("divider", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `divider_widget`, `separator`

### Rating (`rating`)

A row of selectable stars or icons with an optional half step and a numeric value. Use it for a subjective score; use a `Slider` for a continuous quantity and a `SpinBox` when the score must also be typed.

**Properties.** 6 published beyond the four shared: `value`: Float · `allow_half_step`: Bool · `max`: UInt · `star_size`: UInt · `fill`: Float (read-only) · `display_text`: String (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `rating_changed`: UInt

**Commands.** `set_value`, `set_max`

**Rust API.** `create_rating(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("rating", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `rating_widget`, `star_rating`

### EmptyState (`empty_state`)

A placeholder shown when a view has no content: a message, a description and an optional secondary action. Use it inside a list, table or page whose content can be empty; do not use it as a loading indicator — that is `SkeletonLoader` or `Spinner`.

**Properties.** 3 published beyond the four shared: `message`: String · `secondary_action_text`: String · `description`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `action_pressed`

**Commands.** `set_message`, `set_description`

**Rust API.** `create_empty_state(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("empty_state", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `empty_placeholder`

### ColorHistory (`color_history`)

A history of recently used colours rendered as a swatch grid, with add/remove/select and hover signals. Use it beside a colour picker so a user can return to a recent choice; use `Swatch` for a fixed palette and `ColorWell` for one current colour.

**Properties.** 1 published beyond the four shared: `color_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `color_added`: Color · `color_hovered`: Color · `color_removed`: UInt · `color_selected`: Color · `context_menu_requested`: UInt

**Commands.** No commands are published.

**Rust API.** `create_color_history(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("color_history", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ColorWell (`color_well`)

A compact swatch that shows one current colour and emits `clicked` and `picker_requested`. Use it as the collapsed form of a colour field; open a `ColorPicker` or `ColorDialog` when `picker_requested` arrives, and let the well own only the current value.

**Properties.** 2 published beyond the four shared: `color`: Color · `show_border`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `clicked` · `color_changed`: Color · `picker_requested`

**Commands.** `set_color`

**Rust API.** `create_color_well(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("color_well", geometry, text)` and `WidgetFactory::shared()` for the registry.

### FloatingLabel (`floating_label`)

A text field with a label that floats above the input once the field has content or focus. Use it for a compact Material-style form field; use `LineEdit` when a static label above the field fits the form better, and do not use it for multi-line text.

**Properties.** 5 published beyond the four shared: `text`: String · `label`: String · `placeholder`: String · `focused`: Bool · `floating_label_behavior`: Enum (`auto` | `always` | `never`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `text_changed`: String

**Commands.** `set_text`, `set_placeholder`, `set_focused`

**Rust API.** `create_floating_label(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("floating_label", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `floating_input`

### FontPreview (`font_preview`)

A specimen panel that renders a sample string in a chosen family and size. Use it beside a `FontComboBox` or inside a `FontDialog` so the user sees the choice at a readable size; it is a preview only and owns no font selection state.

**Properties.** 3 published beyond the four shared: `font_family`: String · `font_size`: Float · `preview_text`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** `set_font_family`, `set_font_size`, `set_preview_text`

**Rust API.** `create_font_preview(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("font_preview", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `font_viewer`

### ProgressCircle (`progress_circle`)

A circular progress indicator with thickness, colour, diameter, an indeterminate mode and optional centre text. Use it for a compact completion indicator; use `Arc` for a gauge-like sweep and `Spinner` when duration is unknown.

**Properties.** 7 published beyond the four shared: `value`: Float · `thickness`: Float · `progress_color`: Color · `diameter`: UInt · `indeterminate`: Bool · `show_center_text`: Bool · `center_text_format`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `indeterminate_changed`: Bool · `value_changed`: Float

**Commands.** `set_value`, `set_thickness`, `set_indeterminate`

**Rust API.** `create_progress_circle(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("progress_circle", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `circular_progress`

### Icon (`icon`)

A widget that renders a named or source-described icon at a chosen size and colour. Use it for standalone glyphs in toolbars and empty states; when the glyph is part of a label or button, set that control's `icon_source` instead of stacking a separate `Icon`.

**Properties.** 4 published beyond the four shared: `icon_name`: String · `icon_source`: IconSpec · `size`: Float · `color`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** `set_icon_name`, `set_size`

**Rust API.** `create_icon(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("icon", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `icon_widget`, `icon_display`

### Tag (`tag`)

A compact pill label that categorises or filters content, with a tone, an optional icon and a close affordance. Use it for a single read-only category mark; use `Chip` for the multi-item selectable/reorderable row, and `TagInput` when the user creates tags.

**Properties.** 5 published beyond the four shared: `text`: String · `icon`: String · `tone`: Enum (`neutral` | `primary` | `success` | `warning` | `error`) · `selected`: Bool · `closable`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `closed`: String · `selected_changed`: Bool

**Commands.** No commands are published.

**Rust API.** `create_tag(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tag", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `label_tag`

### Clock (`clock`)

An analog clock face showing a time of day, with an optional seconds hand and either a manual time or the system clock as its source. Use it for a clock display; use `Calendar` to pick a date and `TimePicker` to edit a time.

**Properties.** 5 published beyond the four shared: `hour`: UInt · `minute`: UInt · `second`: UInt · `show_seconds`: Bool · `time_source`: Enum (`manual` | `system`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `tick`: UInt

**Commands.** No commands are published.

**Rust API.** `create_clock(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("clock", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `analog_clock`

## 6. Containers and layout primitives

Containers own children: they arrange them, clip them, page them, split them or give them a fixed box. This family includes both the classic tab/splitter/dock shells and the Flutter-style single-child layout boxes (`Center`, `Padding`, `Expanded`, …).

Choose a container for *placement*, not for decoration alone: `ScrollArea` when content can exceed the viewport, `Splitter` when the user resizes panes, `TabWidget` when pages share one slot, `StackedWidget` when the program switches pages, `Accordion`/`CollapsiblePane` to hide secondary sections, and `Flex`/`FlowLayout`/`WrapLayout` for linear or wrapping runs. Do not use a container to implement a model-backed list (use the view family), and do not use `Frame` as a layout engine. A `Spacer` really paints nothing — that is its contract, not a missing implementation.

| Family | Source directory | Controls |
|---|---|---|
| `container` | `src/widget/container_widgets/` | 27 |

### GroupBox (`group_box`)

A titled frame that groups related controls, optionally checkable so the whole group can be enabled or disabled with one state. Use it for a labelled section of a form; use a plain `Frame` when there is no title, and `Accordion` when the group must collapse.

**Properties.** 4 published beyond the four shared: `title`: String · `alignment`: Enum (`left` | `centre` | `right` | `top` | `bottom`) · `checkable`: Bool · `checked`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `toggled`: Bool

**Commands.** `set_title`, `set_checkable`, `set_checked`, `toggle`

**Rust API.** `create_group_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("group_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Splitter (`splitter`)

A draggable divider that splits a container into resizable panes, reporting layout changes and a live preview while dragging. Use it where the user must trade space between two areas; use `split_layout` for the same behaviour under the layout-family name, and a fixed `Flex` when proportions never change.

**Properties.** 2 published beyond the four shared: `orientation`: Enum (`horizontal` | `vertical`) · `pane_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `orientation_changed`: String · `pane_layout_changed`: list of Float · `pane_layout_preview`: list of Float

**Commands.** `set_orientation`

**Rust API.** `create_splitter(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("splitter", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `pane_splitter`

### Toolbox (`tool_box`)

A vertical or horizontal stack of labelled sections that expand one at a time, in exclusive, non-exclusive or popup mode. Use it for a compact tool palette; use `Accordion` for page content sections and `TabWidget` when each section is a full page.

**Properties.** 4 published beyond the four shared: `item_count`: UInt (read-only) · `current_index`: UInt · `orientation`: Enum (`horizontal` | `vertical`) · `mode`: Enum (`exclusive` | `non_exclusive` | `popup`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `current_changed`: UInt · `item_expanded_changed`: mixed (String parts)

**Commands.** `add_item`, `remove_item`, `set_current_index`, `set_orientation`

**Rust API.** `create_tool_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tool_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ScrollArea (`scroll_area`)

A scrollable viewport that clips and offsets one larger child, with independent horizontal and vertical scroll-bar policies and optional sticky regions. Use it when a whole subtree can exceed the visible area; attach a `StickyHeader` companion to keep a band visible, and do not nest scroll areas in the same direction.

**Properties.** 8 published beyond the four shared: `widget_resizable`: Bool · `horizontal_scroll_bar_policy`: Enum (`always_on` | `always_off` | `as_needed`) · `vertical_scroll_bar_policy`: Enum (`always_on` | `always_off` | `as_needed`) · `scroll_position_x`: Int · `scroll_position_y`: Int · `content_width`: UInt · `content_height`: UInt · `sticky_region_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `scroll_position_changed`: Int ×2

**Commands.** `set_widget_resizable`, `set_horizontal_scroll_bar_policy`, `set_vertical_scroll_bar_policy`

**Rust API.** `create_scroll_area(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("scroll_area", geometry, text)` and `WidgetFactory::shared()` for the registry.

### TabWidget (`tab_widget`)

A container of tabbed pages where only the selected page is visible, with closable and movable tabs and a configurable tab position and shape. Use it when each page is a container with its own layout; use `TabView` for lightweight peer pages and `StackedWidget` when the program switches pages without a tab strip.

**Properties.** 9 published beyond the four shared: `tab_count`: UInt (read-only) · `current_index`: UInt · `text`: String · `title`: String · `closable`: Bool · `movable`: Bool · `tab_position`: Enum (`north` | `south` | `west` | `east`) · `tab_shape`: Enum (`rounded` | `triangular` | `rectangular`) · `document_mode`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `context_menu_requested`: String · `current_changed`: UInt · `drag_finished`: String · `tab_bar_clicked`: UInt · `tab_bar_double_clicked`: UInt · `tab_close_requested`: UInt · `tab_moved`: UInt ×2

**Commands.** `add_tab`, `remove_tab`, `set_current_index`, `set_closable`, `set_movable`, `set_tab_position`, `set_tab_shape`

**Rust API.** `create_tab_widget(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tab_widget", geometry, text)` and `WidgetFactory::shared()` for the registry.

### StackedWidget (`stacked_widget`)

A container that shows one child page at a time, selected programmatically by index. Use it for wizard-like or mode-based content where the host decides the page; use `TabWidget` when the user picks pages, and `NavigationStack` when there is a back history.

**Properties.** 2 published beyond the four shared: `widget_count`: UInt (read-only) · `current_index`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `current_changed`: UInt

**Commands.** `add_widget`, `remove_widget`, `set_current_index`

**Rust API.** `create_stacked_widget(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("stacked_widget", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `stacked`

### CollapsiblePane (`collapsible_pane`)

A section with a clickable header that expands or hides one content child. Use it for a single optional detail block; use `Accordion` when several such sections should coordinate, and `GroupBox` when the section is always open.

**Properties.** 2 published beyond the four shared: `title`: String · `collapsed`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `toggled`: Bool

**Commands.** `set_title`, `set_collapsed`, `toggle`

**Rust API.** `create_collapsible_pane(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("collapsible_pane", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `collapsible`

### DockWidget (`dock_widget`)

One dockable panel with a title that can be floated or docked at an edge. Use it for a single tool panel the user can rearrange; a multi-region dock system is not this control, and `MdiArea` is the container for independent child windows rather than dock edges.

**Properties.** 3 published beyond the four shared: `title`: String · `floating`: Bool · `docked`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `dock_location_changed`: String · `features_changed`: String · `top_level_changed`: Bool

**Commands.** `set_title`, `set_floating`, `set_features`, `set_allowed_areas`

**Rust API.** `create_dock_widget(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("dock_widget", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `dock`

### MdiArea (`mdi_area`)

A workspace that hosts multiple independent child windows in sub-window or tabbed view, tracking the active sub-window. Use it for a document-centric desktop app; use `TabWidget` for simple pages and `NavigationStack` for a single linear history.

**Properties.** 3 published beyond the four shared: `subwindow_count`: UInt (read-only) · `active_subwindow`: UInt (read-only) · `view_mode`: Enum (`sub_window_view` | `tabbed`) (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `subwindow_activated`: UInt · `subwindow_title_changed`: mixed (String parts)

**Commands.** `add_subwindow`, `remove_subwindow`, `set_view_mode`, `activate_subwindow`

**Rust API.** `create_mdi_area(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("mdi_area", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `mdi`

### Stepper (`stepper`)

A numeric field flanked by plus and minus buttons that nudge the value by fixed steps. Use it for a small integer quantity (counts, quantities in a cart); use `SpinBox` when the user may also type the value, and `NumberPicker` for a wheel.

**Properties.** 4 published beyond the four shared: `value`: Int · `minimum`: Int · `maximum`: Int · `step`: Int — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `value_changed`: Int

**Commands.** `set_value`, `set_minimum`, `set_maximum`, `set_step`

**Rust API.** `create_stepper(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("stepper", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `stepper_widget`, `step_control`

### MasonryLayout (`masonry_layout`)

A Pinterest-style waterfall grid that places items in the shortest column, with a column count and spacing. Use it for variable-height cards (photos, notes); use `FlowLayout`/`WrapLayout` when rows must align, and `Grid` when every cell has the same size.

**Properties.** 3 published beyond the four shared: `column_count`: UInt · `item_count`: UInt (read-only) · `item_spacing`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `item_clicked`: UInt

**Commands.** `set_column_count`

**Rust API.** `create_masonry_layout(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("masonry_layout", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `waterfall_layout`

### SafeArea (`safe_area`)

A container that insets its content by top, bottom, left and right amounts so notches, status bars and home indicators do not overlap it. Use it at the root of a mobile screen; set the insets from the platform query rather than hard-coding device metrics.

**Properties.** 4 published beyond the four shared: `top_inset`: Float · `bottom_inset`: Float · `left_inset`: Float · `right_inset`: Float — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** `set_top_inset`, `set_bottom_inset`, `set_left_inset`, `set_right_inset`

**Rust API.** `create_safe_area(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("safe_area", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `safe_area_insets`

### Carousel (`carousel`)

A horizontal swipeable page carousel with dot, bar or numeric indicators, optional looping and autoplay. Use it for a small set of full-width pages; pair it with `Pagination` when the page index must live outside the carousel, and use `TabView` when pages are selected by tabs rather than swipes.

**Properties.** 7 published beyond the four shared: `current_index`: UInt · `item_count`: UInt (read-only) · `current_page_title`: String (read-only) · `loop`: Bool · `autoplay_interval`: UInt · `indicator_style`: Enum (`dots` | `bars` | `numeric` | `hidden`) · `indicator_position`: Enum (`bottom` | `top` | `left` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `page_changed`: UInt

**Commands.** No commands are published.

**Rust API.** `create_carousel(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("carousel", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `carousel_widget`, `swipe_view`

### Spacer (`spacer`)

An empty, invisible area that consumes free space in a linear layout, with a flex weight, a base size and an optional paint hint. Use it to push siblings apart; its painting nothing is the contract, so do not expect a background or border from it.

**Properties.** 4 published beyond the four shared: `flex`: UInt · `base_size`: UInt · `paint_hint`: Bool · `widget`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** No commands are published.

**Rust API.** `create_spacer(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("spacer", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `flexible_space`

### Center (`center`)

A single-child box that centres its content, optionally at a fraction of the parent's width and height. Use it to centre a form or a message; use `Align` for one of nine anchors other than the centre.

**Properties.** 6 published beyond the four shared: `content`: String · `content_width`: UInt · `content_height`: UInt · `width_factor`: Float · `height_factor`: Float · `widget`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** No commands are published.

**Rust API.** `create_center(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("center", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `centre`, `center_widget`

### Align (`align`)

A single-child box that places its content at one of nine anchor positions. Use it to pin a child to a corner or edge; use `Center` for the centre case and `Padding` when the offset is a fixed inset rather than an anchor.

**Properties.** 5 published beyond the four shared: `content`: String · `content_width`: UInt · `content_height`: UInt · `alignment`: Enum (`top_left` | `top_centre` | `top_right` | `centre_left` | `centre` | `centre_right` | `bottom_left` | `bottom_centre` | `bottom_right`) · `widget`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** No commands are published.

**Rust API.** `create_align(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("align", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `alignment_widget`

### SizedBox (`sized_box`)

A box with a fixed width and height that contains one child. Use it to reserve an exact slot (`box_width`, `box_height`); use `AspectRatio` when one extent should follow the other, and `IntrinsicWidth` when the size should follow the content.

**Properties.** 6 published beyond the four shared: `content`: String · `content_width`: UInt · `content_height`: UInt · `box_width`: UInt · `box_height`: UInt · `widget`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** No commands are published.

**Rust API.** `create_sized_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("sized_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `size_box`

### Padding (`padding`)

A single-child box that insets its content by four independent edge paddings. Use it for spacing that belongs to the content rather than to the parent; use the parent's `spacing`/`gap` when the gap is between siblings.

**Properties.** 8 published beyond the four shared: `content`: String · `content_width`: UInt · `content_height`: UInt · `padding_top`: UInt · `padding_right`: UInt · `padding_bottom`: UInt · `padding_left`: UInt · `widget`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** No commands are published.

**Rust API.** `create_padding(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("padding", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `padding_widget`

### Expanded (`expanded`)

A child that takes the leftover room along the main axis, weighted by `flex`, and must be placed inside a linear layout. Use it to make one child absorb slack; use `Flexible` when the child may also stop at its preferred size instead of filling.

**Properties.** 5 published beyond the four shared: `content`: String · `content_width`: UInt · `content_height`: UInt · `flex`: UInt · `widget`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** No commands are published.

**Rust API.** `create_expanded(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("expanded", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `expand`

### Flexible (`flexible`)

A child that may shrink to its floor but only grows up to its preferred size, with `fit` choosing loose or tight constraints. Use it when a sibling should yield space but not necessarily fill it; use `Expanded` when it must fill.

**Properties.** 6 published beyond the four shared: `content`: String · `content_width`: UInt · `content_height`: UInt · `flex`: UInt · `fit`: Enum (`loose` | `tight`) · `widget`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** No commands are published.

**Rust API.** `create_flexible(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("flexible", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `flex_child`

### IntrinsicWidth (`intrinsic_width`)

A box whose width is its content's own preferred width, so a field or label does not stretch. Use it for shrink-to-fit columns; use `SizedBox` for an exact width and `AspectRatio` when height must follow width.

**Properties.** 4 published beyond the four shared: `content`: String · `content_width`: UInt · `content_height`: UInt · `widget`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** No commands are published.

**Rust API.** `create_intrinsic_width(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("intrinsic_width", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `shrink_wrap`

### AspectRatio (`aspect_ratio`)

A box that derives one extent from the other through a fixed ratio, optionally deferring to the parent's constraint. Use it for media tiles and thumbnails; use `SizedBox` when both extents are known and `IntrinsicWidth` when content should drive width.

**Properties.** 6 published beyond the four shared: `content`: String · `content_width`: UInt · `content_height`: UInt · `ratio`: Float · `respect_parent`: Bool · `widget`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** No commands are published.

**Rust API.** `create_aspect_ratio(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("aspect_ratio", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `ratio_box`

### Flex (`flex`)

A linear layout that distributes children along a row or column by weight, with wrap, justify-content, align-items and gap. Use it for most one-dimensional layouts; use `Grid` for two dimensions and `FlowLayout`/`WrapLayout` when runs must wrap by measured size.

**Properties.** 6 published beyond the four shared: `item_count`: UInt (read-only) · `direction`: Enum (`row` | `row_reverse` | `column` | `column_reverse`) · `wrap`: Enum (`no_wrap` | `wrap` | `wrap_reverse`) · `justify_content`: Enum (`flex_start` | `flex_end` | `center` | `space_between` | `space_around` | `space_evenly`) · `align_items`: Enum (`stretch` | `flex_start` | `flex_end` | `center` | `baseline`) · `gap`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `item_clicked`: UInt

**Commands.** No commands are published.

**Rust API.** `create_flex(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("flex", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `flex_box`

### FlowLayout (`flow_layout`)

A layout that places children in reading order and starts a new run when the line is full, sizing each run by its children's own hints and following the writing mode. Use it for chips, tags and tool buttons that should flow; use `WrapLayout` when runs are a fixed-size grid of cells.

**Properties.** 6 published beyond the four shared: `item_count`: UInt (read-only) · `direction`: Enum (`horizontal` | `vertical`) · `alignment`: Enum (`start` | `center` | `end` | `space_between` | `space_around`) · `spacing`: UInt · `padding`: UInt · `wrap`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `item_clicked`: UInt

**Commands.** No commands are published.

**Rust API.** `create_flow_layout(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("flow_layout", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `flow`

### WrapLayout (`wrap_layout`)

A layout that places fixed-size children and wraps them onto the next run when full, with independent spacing and run spacing. Use it for a gallery of equal cells; use `FlowLayout` when children should keep their own measured size and follow the writing mode.

**Properties.** 6 published beyond the four shared: `item_count`: UInt (read-only) · `direction`: Enum (`horizontal` | `vertical`) · `alignment`: Enum (`start` | `center` | `end` | `space_between` | `space_around`) · `spacing`: UInt · `run_spacing`: UInt · `padding`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `item_clicked`: UInt

**Commands.** No commands are published.

**Rust API.** `create_wrap_layout(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("wrap_layout", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `wrap`

### Accordion (`accordion`)

A vertical stack of collapsible sections, in single-expand or multiple-expand mode. Use it for FAQ-style or settings groups; use `CollapsiblePane` for one independent section, and `ToolBox` when the sections are a tool palette rather than page content.

**Properties.** 3 published beyond the four shared: `section_count`: UInt (read-only) · `single_expand`: Bool · `expanded_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `section_toggled`: mixed (String parts)

**Commands.** No commands are published.

**Rust API.** `create_accordion(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("accordion", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `collapsible_list`

### Splitter (`split_layout`)

The layout-family spelling of `Splitter`: a draggable divider between panes, with orientation and pane count. Use it when the split belongs to the layout description rather than to a widget subtree; it reports the same three layout signals as `Splitter`.

**Properties.** 2 published beyond the four shared: `orientation`: Enum (`horizontal` | `vertical`) · `pane_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `orientation_changed`: String · `pane_layout_changed`: list of Float · `pane_layout_preview`: list of Float

**Commands.** `set_orientation`

**Rust API.** `create_split_layout(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("split_layout", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `split_view`

## 7. Model-backed item views

Views present a model: rows, columns, trees and the chrome around them. They own scrolling, selection, focus and (for some) editing, and they ask a host-supplied data source only for the rows currently visible.

Choose the view that matches the model's *shape*: a flat list is `ListView` or `VirtualList`, a tree is `TreeView`, a grid is `Table`, `GridTable` or `VirtualTable`, and a tree with columns is `TreeTable`. Use `DataGrid` when sorting, filtering and frozen columns are part of the feature, and `QueryBuilder`/`PropertyGrid`/`PropertiesPanel` when the data itself is a set of conditions or named properties. Do not use `ListBox` for thousands of rows, `Table` for hierarchical data, or a `ListView` as a form: views do not host arbitrary widgets in their rows in this crate.

| Family | Source directory | Controls |
|---|---|---|
| `view` | `src/widget/view_widgets/` | 15 |

### ListView (`list_view`)

A model-backed list with a view mode (list, icon, details, thumbnails), a focused row and a selection. Use it for rows that carry a model or a delegate; use `ListBox` for a small plain-text list and `VirtualList` when the model is large enough to need windowed projection.

**Properties.** 7 published beyond the four shared: `has_model`: Bool (read-only) · `row_count`: UInt (read-only) · `focused_row`: UInt · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `view_mode`: Enum (`list` | `icon` | `details` | `thumbnails`) · `alignment`: Enum (`left` | `centre` | `right`) · `icon_visible_in_list_mode`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `context_menu_requested`: String · `focused_row_changed`: UInt or null · `row_activated`: UInt · `row_reordered`: UInt ×2 · `selection_changed`: UInt

**Commands.** `clear_selection`, `clear_focused_row`

**Rust API.** `create_list_view(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("list_view", geometry, text)` and `WidgetFactory::shared()` for the registry.

### TreeView (`tree_view`)

A hierarchical view that expands and collapses nodes, with a focused node, a selected node, lazy loading and an animated expansion. Use it for a tree model; use `TreeTable` when each row also needs columns, and `ColumnView` is the alias role for the same tree family rather than a second control.

**Properties.** 10 published beyond the four shared: `has_model`: Bool (read-only) · `node_count`: UInt (read-only) · `focused_node`: UInt · `selected_node`: UInt (read-only) · `has_delegate`: Bool (read-only) · `expanded_focused_node`: Bool (read-only) · `expansion_animated`: Bool · `lazy_loading`: Bool · `loading_indicator_visible`: Bool · `fetch_timeout_ms`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `activated`: UInt · `collapsed`: UInt · `expanded`: UInt · `fetch_finished`: mixed (String parts) · `fetch_started`: list of UInt · `focused_node_changed`: UInt or null · `node_double_clicked`: UInt · `selection_changed`: UInt

**Commands.** `clear_selection`, `clear_focused_node`, `expand`, `collapse`, `toggle`

**Rust API.** `create_tree_view(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tree_view", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Table (`table_widget`)

A table of cells with a header, a current row, selection modes and an edit-request signal. Use it for a flat grid of modest size; use `DataGrid` when sorting, filtering and frozen columns are required, and `VirtualTable` for large models.

**Properties.** 8 published beyond the four shared: `has_model`: Bool (read-only) · `has_delegate`: Bool (read-only) · `row_count`: UInt (read-only) · `column_count`: UInt (read-only) · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `header_visible`: Bool · `editable`: Bool · `edit_triggers`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `context_menu_requested`: String · `edit_requested`: UInt ×2 · `focused_row_changed`: UInt or null · `row_activated`: UInt · `selection_changed`: UInt

**Commands.** `clear_selection`, `clear_focused_row`

**Rust API.** `create_table_widget(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("table_widget", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Table (`table`)

The `table` factory name for the same table control registered as `table_widget`; both names accept the same property and event contract. Use whichever spelling the rest of your host already uses, and do not register both for one widget.

**Properties.** 8 published beyond the four shared: `has_model`: Bool (read-only) · `has_delegate`: Bool (read-only) · `row_count`: UInt (read-only) · `column_count`: UInt (read-only) · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `header_visible`: Bool · `editable`: Bool · `edit_triggers`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `context_menu_requested`: String · `edit_requested`: UInt ×2 · `focused_row_changed`: UInt or null · `row_activated`: UInt · `selection_changed`: UInt

**Commands.** `clear_selection`, `clear_focused_row`

**Rust API.** `create_table(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("table", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Table (`data_grid`)

A data grid with a data source, sorting, filtering, frozen columns, row height and column width, selection behaviour and a summary. Use it when sorting/filtering are part of the feature; use `GridTable` for a lighter grid and `TreeTable` when rows are hierarchical.

**Properties.** 18 published beyond the four shared: `has_data_source`: Bool (read-only) · `row_count`: UInt (read-only) · `column_count`: UInt (read-only) · `scroll_row`: UInt · `scroll_column`: UInt · `row_height`: UInt · `column_width`: UInt · `frozen_columns`: UInt · `sort_spec_count`: UInt (read-only) · `filter_count`: UInt (read-only) · `sort_specs`: String · `filters`: String · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) · `selection_behavior`: Enum (`cells` | `rows` | `columns`) · `sort_indicator_visible`: Bool · `section_resize_mode`: Enum (`fixed` | `stretch` | `interactive`) · `current_cell`: String (read-only) · `summary_text`: String (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `context_menu_requested`: String · `edit_requested`: UInt ×2 · `filter_changed`: String · `header_clicked`: UInt · `page_applied`: UInt · `sort_changed`: mixed (String parts) · `visible_window_changed`: UInt ×4

**Commands.** `set_data_source`, `clear_data_source`, `set_scroll_row`, `set_scroll_column`, `set_row_height`, `set_column_width`, `set_frozen_columns`

**Rust API.** `create_data_grid(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("data_grid", geometry, text)` and `WidgetFactory::shared()` for the registry.

### TreeTable (`tree_table`)

A hierarchical table: rows form a tree with expand/collapse, and each row carries column cells, with sort role, case sensitivity and a windowed or eager projection. Use it for a tree whose nodes also have tabular attributes; use `TreeView` for a pure tree and `Table` for flat rows.

**Properties.** 15 published beyond the four shared: `has_model`: Bool (read-only) · `row_count`: UInt (read-only) · `column_count`: UInt (read-only) · `selected_row`: UInt · `row_height`: UInt · `column_width`: UInt · `header_visible`: Bool · `expands_on_double_click`: Bool · `middle_click_expands`: Bool · `sort_role`: Enum (`text` | `number`) · `case_sensitive`: Bool · `projection_strategy`: Enum (`windowed` | `eager`) · `lazy_loading`: Bool · `loading_indicator_visible`: Bool · `fetch_timeout_ms`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `context_menu_requested`: String · `expansion_changed`: mixed (String parts) · `fetch_finished`: mixed (String parts) · `fetch_started`: list of UInt · `projection_changed`: UInt · `selection_changed`: UInt or null · `sort_changed`: mixed (String parts)

**Commands.** `set_model`, `clear_model`, `expand_row`, `collapse_row`, `select_row`

**Rust API.** `create_tree_table(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tree_table", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Table (`virtual_table`)

A table over a data source that projects only a visible window, with overscan rows and columns and a wheel scroll mode. Use it for large, remotely backed tables; use `DataGrid` when sorting/filtering are needed and the model is local.

**Properties.** 12 published beyond the four shared: `has_data_source`: Bool (read-only) · `row_count`: UInt (read-only) · `column_count`: UInt (read-only) · `scroll_row`: UInt · `scroll_column`: UInt · `row_height`: UInt · `column_width`: UInt · `overscan_rows`: UInt · `overscan_columns`: UInt · `visible_window`: String (read-only) · `wheel_scroll_mode`: Enum (`per_row` | `per_page`) · `header_visible`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `selection_changed`: (UInt, UInt) or null · `visible_window_changed`: UInt ×4

**Commands.** `set_data_source`, `clear_data_source`, `set_scroll_row`, `set_scroll_column`, `set_row_height`, `set_column_width`, `set_overscan_rows`, `set_overscan_columns`, `fetch_visible_window`

**Rust API.** `create_virtual_table(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("virtual_table", geometry, text)` and `WidgetFactory::shared()` for the registry.

### DataView (`virtual_list`)

A single-column virtual list over a data source, with a visible window, overscan and multi-selection. Use it for long lists of uniform rows; use `ListView` when rows need view modes or delegates, and `ListBox` for a short materialized list.

**Properties.** 8 published beyond the four shared: `has_data_source`: Bool (read-only) · `row_count`: UInt (read-only) · `scroll_row`: UInt · `row_height`: UInt · `overscan`: UInt · `selected_row`: UInt (read-only) · `selected_rows`: NumberList (read-only) · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `drag_finished`: String · `selected_rows_changed`: list of UInt · `selection_changed`: UInt or null · `visible_window_changed`: UInt ×2

**Commands.** `clear_data_source`, `fetch_visible_rows`

**Rust API.** `create_virtual_list(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("virtual_list", geometry, text)` and `WidgetFactory::shared()` for the registry.

### DataView (`data_view`)

The `data_view` spelling of `VirtualList`; it presents one column of model rows and has no display-mode switching. Use it when a host wants the virtual-list contract under the generic data-view name, and do not expect a second implementation.

**Properties.** 8 published beyond the four shared: `has_data_source`: Bool (read-only) · `row_count`: UInt (read-only) · `scroll_row`: UInt · `row_height`: UInt · `overscan`: UInt · `selected_row`: UInt (read-only) · `selected_rows`: NumberList (read-only) · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `drag_finished`: String · `selected_rows_changed`: list of UInt · `selection_changed`: UInt or null · `visible_window_changed`: UInt ×2

**Commands.** `clear_data_source`, `fetch_visible_rows`

**Rust API.** `create_data_view(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("data_view", geometry, text)` and `WidgetFactory::shared()` for the registry.

### GridTable (`grid_table`)

A feature-rich virtualized table with grid lines, headers, sorting, selection and a data source. Use it for a spreadsheet-like surface inside a form; use `DataGrid` for filter/frozen-column features and pair it with `GridViewHeader` when the header must be shared outside the grid.

**Properties.** 12 published beyond the four shared: `has_data_source`: Bool (read-only) · `row_count`: UInt (read-only) · `column_count`: UInt (read-only) · `scroll_row`: UInt · `scroll_column`: UInt · `row_height`: UInt · `selection_mode`: Enum (`none` | `cell` | `row` | `column`) · `sort_spec_count`: UInt (read-only) · `selected_cell`: String (read-only) · `sorting_enabled`: Bool · `sort_role`: String · `auto_refresh`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `cell_double_clicked`: UInt ×2 · `cell_selected`: UInt ×2 · `context_menu_requested`: String · `edit_requested`: UInt ×2 · `header_clicked`: UInt · `sort_changed`: mixed (String parts)

**Commands.** `set_data_source`, `clear_data_source`, `set_scroll_row`, `set_scroll_column`, `toggle_sort_column`, `clear_selection`

**Rust API.** `create_grid_table(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("grid_table", geometry, text)` and `WidgetFactory::shared()` for the registry.

### QueryBuilder (`query_builder`)

A builder that edits a recursive filter as rows of field, operator and operand, with a conjunction, live update and commit delay. Use it for a structured filter whose text must be exportable; use `FilterRow` beside a table for a quick one-line filter, and a `SearchBox` for free-text search.

**Properties.** 8 published beyond the four shared: `row_count`: UInt (read-only) · `incomplete_row_count`: UInt (read-only) · `field_count`: UInt (read-only) · `conjunction`: Enum (`and` | `or`) · `active_row`: UInt · `live_update`: Bool · `commit_delay_ms`: UInt · `has_pending_commit`: Bool (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `query_changed`: String · `query_committed`: String · `row_field_clicked`: UInt · `row_operator_clicked`: UInt · `rows_changed`: list of Records

**Commands.** `set_fields`, `add_row`, `remove_row`, `toggle_conjunction`

**Rust API.** `create_query_builder(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("query_builder", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `filter_builder`

### PropertiesPanel (`properties_panel`)

A categorized property editor that lays out rows in a grid and reports edits, file-picker requests and editor opens. Use it for an inspector beside a canvas; use `PropertyGrid` for the flatter two-column form and `PropertiesPanel` when categories matter.

**Properties.** 3 published beyond the four shared: `property_count`: UInt (read-only) · `description_mode`: Enum (`tooltip` | `status_bar` | `hidden`) · `edit_trigger`: Enum (`single_click` | `double_click`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `editor_opened`: String · `file_picker_requested`: String · `property_changed`: String ×2

**Commands.** No commands are published.

**Rust API.** `create_properties_panel(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("properties_panel", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `property_panel`

### PropertyGrid (`property_grid`)

A two-column editor listing named properties with an editable value cell, a selected row and per-cell change signals. Use it for a compact key/value inspector; use `PropertiesPanel` when rows must be grouped into categories.

**Properties.** 2 published beyond the four shared: `property_count`: UInt (read-only) · `selected_index`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `cell_changed`: mixed (String parts) · `context_menu_requested`: String · `selected`: UInt

**Commands.** `add_property`, `set_value`, `clear`, `set_selected_index`

**Rust API.** `create_property_grid(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("property_grid", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `inspector`

### ImageGallery (`image_gallery`)

An image browser with a main image, a thumbnail strip, wheel modes and fullscreen requests. Use it for a set of photos with the current index visible; use `ImageView` for one image and pair it with `ThumbnailCache` when the thumbnail strip is large.

**Properties.** 7 published beyond the four shared: `current_index`: UInt · `thumbnail_size`: UInt · `show_thumbnails`: Bool · `strip_offset`: UInt · `strip_scroll_enabled`: Bool · `wheel_mode`: Enum (`strip` | `navigate` | `zoom`) · `arrow_key_navigation_enabled`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `fullscreen_requested`: UInt · `image_changed`: UInt · `selection_changed`: list of UInt · `strip_scrolled`: UInt

**Commands.** `set_current_index`

**Rust API.** `create_image_gallery(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("image_gallery", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `gallery`

### GridViewHeader (`grid_view_header`)

A shared header strip for a grid or table: sortable and resizable sections plus a filter affordance. Use it when the header must live outside the scrolling body, and report `section_clicked`, `section_resized` and `filter_clicked` to the grid that owns the model.

**Properties.** 6 published beyond the four shared: `columns`: StringList · `column_count`: UInt (read-only) · `sort_indicator_visible`: Bool · `resize_mode`: Enum (`fixed` | `stretch` | `stretch_last` | `interactive`) · `filter_icon_visible`: Bool · `sort_columns`: String (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `filter_clicked`: UInt · `section_clicked`: UInt · `section_resized`: UInt ×2

**Commands.** No commands are published.

**Rust API.** `create_grid_view_header(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("grid_view_header", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `table_header_bar`

## 8. Charts and gauges

The chart family is the control layer over the shared drawing engine: line, bar, pie and spark charts plus the radial `Gauge`. The engine owns axes, ticks, margins and legend placement; the controls own the model, signals and kind semantics.

Choose `LineChart` for an ordered series, `BarChart` for categorical comparison, `PieChart` for a part-to-whole snapshot with few slices, `Sparkline` for a compact inline trend with no axes, and `Gauge` for one value against a range. Do not use a chart for OHLC market data (the finance family), for a heat map or radar plot (the special family), or for a single number — a `Label` or `LCDNumber` is clearer.

| Family | Source directory | Controls |
|---|---|---|
| `chart` | `src/widget/chart_widgets/` | 5 |

### LineChart (`line_chart`)

A cartesian line chart for one or more ordered series, drawn through the shared chart engine. Use it for trends over an ordered axis; use `Sparkline` when the trend is inline and no axes are needed, and `IndicatorChart` for technical indicators.

**Properties.** 1 published beyond the four shared: `stroke_width`: Float — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `point_clicked`: UInt ×2 · `point_hovered`: UInt ×2 · `point_unhovered`: UInt ×2

**Commands.** `set_stroke_width`

**Rust API.** `create_line_chart(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("line_chart", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `line_graph`

### Sparkline (`sparkline`)

A compact inline trend with no axes, no grid and no margins, built from a value list with an end marker and hover reporting. Use it in a table cell or a KPI tile; use `LineChart` when axes or zoom matter. It deliberately does not share the chart engine, because it has none of the axes, grid, ticks or margins the engine exists to unify.

**Properties.** 7 published beyond the four shared: `data`: NumberList · `stroke_width`: Float · `show_end_value`: Bool · `fill`: Bool · `show_points`: Bool · `show_last_marker`: Bool · `capacity`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `last_value_clicked`: Float · `value_hovered`: mixed (String parts)

**Commands.** `set_stroke_width`

**Rust API.** `create_sparkline(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("sparkline", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `sparkline_chart`

### BarChart (`bar_chart`)

A vertical bar chart for categorical comparison, with bar spacing and per-bar hover/click signals. Use it for a small number of categories; use `LineChart` for an ordered series, `PieChart` for part-to-whole and `RadarChart` for multi-axis profiles.

**Properties.** 1 published beyond the four shared: `bar_spacing`: Float — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `bar_clicked`: UInt · `bar_hovered`: UInt · `bar_unhovered`: UInt

**Commands.** `set_bar_spacing`

**Rust API.** `create_bar_chart(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("bar_chart", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `bar_graph`

### PieChart (`pie_chart`)

A circular chart with coloured sectors, optionally as a donut, reporting the slice under the pointer and the clicked slice. Use it for a part-to-whole snapshot with few slices; use `BarChart` when the categories are many or close in value.

**Properties.** 1 published beyond the four shared: `donut`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `slice_clicked`: UInt · `slice_hovered`: UInt · `slice_unhovered`: UInt

**Commands.** `set_donut`

**Rust API.** `create_pie_chart(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("pie_chart", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `pie_graph`

### Gauge (`gauge`)

A radial dial showing one value against a range, with bands, ticks, a needle style and an optional unit label. Use it for a single measurement with a meaningful scale; use `Meter` for a linear gauge and `Arc`/`ProgressCircle` for completion.

**Properties.** 14 published beyond the four shared: `value`: Float · `minimum`: Float · `maximum`: Float · `label`: String · `unit`: String · `zone_count`: UInt (read-only) · `animated`: Bool · `thickness`: UInt · `bands`: StringList · `tick_count`: UInt · `start_angle`: Float · `end_angle`: Float · `needle_style`: Enum (`none` | `line` | `triangle`) · `unit_label`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `band_clicked`: String · `value_changed`: Float

**Commands.** No commands are published.

**Rust API.** `create_gauge(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("gauge", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `dial_gauge`

## 9. Menus, toolbars and command surfaces

This family holds the command chrome: menu bars, drop-down and context menus, toolbars, status bars, individual actions and the buttons that open a menu or split into two actions.

Choose `MenuBar` for the application's top-level command groups, `Menu` or `ContextMenu` for a list of commands opened from a button or a secondary click, `ToolBar` for frequently used actions, `StatusBar` for a short status line, and `Action` when a command must be described once and rendered in several places. Do not use a menu as a data list with selection state (that is a view), and do not store command state in a `Menu` — annotate the `Action` and keep the model in the host.

| Family | Source directory | Controls |
|---|---|---|
| `menu_toolbar` | `src/widget/menu_toolbar/` | 10 |

### Action (`action`)

A command descriptor that can be rendered by menus and toolbars: text, icon, shortcut, checkable/checked state, a separator flag and a command id. Use it when one command must appear in several surfaces; do not use it as a visible button by itself — the surfaces instantiate the concrete control.

**Properties.** 8 published beyond the four shared: `text`: String · `icon_text`: String · `shortcut`: String · `checkable`: Bool · `checked`: Bool · `icon_source`: IconSpec · `separator`: Bool (read-only) · `command_id`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `changed` · `hovered` · `toggled`: Bool · `triggered`: Bool

**Commands.** `set_text`, `set_checkable`, `set_checked`, `trigger`

**Rust API.** `create_action(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("action", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `command_action`

### Menu (`menu`)

A drop-down list of commands and submenus, with hover, action-triggered, submenu-requested and about-to-show/hide signals. Use it for commands opened from a menu bar, button or context click; use `DropdownMenu` when the popup selects a value and `MenuButton` when a button owns the menu.

**Properties.** 6 published beyond the four shared: `title`: String · `item_count`: UInt (read-only) · `hovered_index`: UInt (read-only) · `direction`: Enum (`ltr` | `rtl`) · `show_heading`: Bool · `column_count`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `about_to_hide` · `about_to_show` · `action_triggered`: String · `submenu_requested`: String · `tearoff_requested` · `triggered`: String · `triggered_index`: UInt

**Commands.** `clear`, `add_action`, `add_separator`

**Rust API.** `create_menu(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("menu", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Menu (`context_menu`)

`ContextMenu` is the role spelling of `Menu` (`pub type ContextMenu = Menu`), registered under its own name for a menu opened at the pointer. Use it for a secondary-click menu; it is not a second menu implementation, so properties and events are exactly the `Menu` set.

**Properties.** 6 published beyond the four shared: `title`: String · `item_count`: UInt (read-only) · `hovered_index`: UInt (read-only) · `direction`: Enum (`ltr` | `rtl`) · `show_heading`: Bool · `column_count`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `about_to_hide` · `about_to_show` · `action_triggered`: String · `submenu_requested`: String · `tearoff_requested` · `triggered`: String · `triggered_index`: UInt

**Commands.** `clear`, `add_action`, `add_separator`

**Rust API.** `create_context_menu(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("context_menu", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `context_actions`

### MenuBar (`menu_bar`)

A horizontal strip of top-level menu titles that tracks the active and hovered entry. Use it for the application's command groups at the top of a window; use `ToolBar` for frequently used commands and `MenuButton` for a single drop-down in a page.

**Properties.** 3 published beyond the four shared: `entry_count`: UInt (read-only) · `active_index`: UInt (read-only) · `hovered_index`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `entry_hovered`: String · `triggered`: String · `triggered_entry`: mixed (String parts)

**Commands.** `clear`, `add_menu`, `remove_menu`

**Rust API.** `create_menu_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("menu_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ToolBar (`tool_bar`)

A strip of icon buttons and separators, horizontal or vertical, movable, floatable and reporting orientation and visibility changes. Use it for frequently used commands; use `RibbonBar` when commands need labelled groups and tabs, and `ToolButton` for the individual items.

**Properties.** 5 published beyond the four shared: `orientation`: Enum (`horizontal` | `vertical`) · `icon_size`: Float · `movable`: Bool · `floatable`: Bool · `item_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `action_triggered`: String · `customize_requested` · `float_changed`: Bool · `orientation_changed`: Bool · `top_level_changed`: Bool · `visibility_changed`: Bool

**Commands.** `clear`, `add_action`, `add_separator`

**Rust API.** `create_tool_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tool_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

### StatusBar (`status_bar`)

A short status line at the bottom of a window, with a message and a size grip. Use it for transient status text; `Snackbar` is also registered with this kind but is a floating notice, so do not confuse the two — set `message` here and let the host update it.

**Properties.** 1 published beyond the four shared: `message`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `message_changed`: String · `size_grip_dragged`: Int ×2

**Commands.** `set_message`

**Rust API.** `create_status_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("status_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `status`

### ToolButton (`tool_button`)

A compact icon-and-caption button for toolbars, with a checked state and click/toggle signals. Use it inside a `ToolBar` or a `SplitButton` group; use a full `Button` when the caption is the primary target and `ToggleButton` when the button owns a popup.

**Properties.** 3 published beyond the four shared: `text`: String · `checked`: Bool · `icon_source`: IconSpec — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `clicked`: Bool · `toggled`: Bool · `triggered`

**Commands.** `set_text`, `set_checked`

**Rust API.** `create_tool_button(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tool_button", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `toolbar_button`

### ToolButton (`split_button`)

A tool button split into a primary action and a menu of alternatives, with a popup direction, a row height and a scroll position. Use it when one action is default and several variants share space; use `MenuButton` when every click opens the menu.

**Properties.** 7 published beyond the four shared: `text`: String · `action_count`: UInt (read-only) · `menu_open`: Bool · `row_height`: UInt · `popup_direction`: Enum (`down` | `up` | `auto`) · `max_menu_height`: UInt · `menu_scroll`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `action_selected`: String · `menu_toggled`: Bool · `triggered`: String

**Commands.** `add_action`, `open_menu`, `close_menu`, `trigger_primary`

**Rust API.** `create_split_button(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("split_button", geometry, text)` and `WidgetFactory::shared()` for the registry.

### DropdownMenu (`dropdown_menu`)

A drop-down selector with an item list, a selected index, an expanded state and submenu requests. Use it when the popup chooses a value from a list; use `ComboBox` when the field also displays the current value in a text box, and `Menu` when the items are commands.

**Properties.** 4 published beyond the four shared: `item_count`: UInt (read-only) · `selected_index`: UInt · `expanded`: Bool · `alignment`: Enum (`left` | `centre` | `right`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `about_to_hide` · `about_to_show` · `item_selected`: String · `item_toggled`: String · `submenu_requested`: String

**Commands.** `set_selected_index`, `set_expanded`

**Rust API.** `create_dropdown_menu(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("dropdown_menu", geometry, text)` and `WidgetFactory::shared()` for the registry.

### MenuButton (`menu_button`)

A button that opens a menu on click, with an icon, an item count and about-to-show/hide plus item-triggered signals. Use it for a single drop-down command surface in a page; use `SplitButton` when the primary action must also be clickable.

**Properties.** 4 published beyond the four shared: `text`: String · `icon_source`: IconSpec · `item_count`: UInt (read-only) · `expanded`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `about_to_hide` · `about_to_show` · `clicked` · `item_triggered`: UInt · `submenu_requested`: String

**Commands.** `set_text`, `set_expanded`

**Rust API.** `create_menu_button(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("menu_button", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `dropdown_button`

## 10. Dialogs and transient panels

Dialogs are separate surfaces with their own result: message and confirmation boxes, file/colour/font/input pickers, progress and wizard dialogs, plus the non-modal bottom sheets, popovers and tooltips.

Choose `MessageBox` or the alert/confirm dialogs for a decision, `Dialog` when you need a titled shell with one content widget, `FileDialog`/`ColorDialog`/`FontDialog`/`InputDialog` for the standard pickers, `ProgressDialog` for cancellable long work, and `WizardDialog` for ordered steps. `BottomSheet`, `ModalBottomSheet`, `Popover` and `Tooltip` are for transient content that does not own the window. Do not use a dialog for a non-blocking notice (`Banner`, `Toast`, `Snackbar`) and do not nest a modal inside another modal's own modal stack — route it through `ModalHost`.

| Family | Source directory | Controls |
|---|---|---|
| `dialog` | `src/widget/dialog/` | 17 |

### Dialog (`dialog`)

A titled dialog shell that hosts one content widget, with modal, accepted/rejected/result and help signals. Use it when you need your own content inside a dialog frame; use `MessageBox` and the alert/confirm dialogs for standard decisions, and `PopupWindow` for chrome-only floating content.

**Properties.** 7 published beyond the four shared: `title`: String · `modal`: Bool · `has_content`: Bool (read-only) · `result`: Int (read-only) · `accepted`: Bool (read-only) · `rejected`: Bool (read-only) · `closable`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `accepted` · `closed` · `finished`: Int · `help_requested` · `opened` · `rejected`

**Commands.** `accept`, `reject`

**Rust API.** `create_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

### MessageBox (`message_box`)

A modal message with an icon, informative and detailed text, an optional check box and a fixed set of response buttons. Use it for a short decision or acknowledgement; use `AlertDialog`/`ConfirmDialog` for a more styled notice and `Dialog` when the content is custom.

**Properties.** 10 published beyond the four shared: `title`: String · `text`: String · `icon`: Enum (`none` | `information` | `question` | `warning` | `critical`) · `modal`: Bool · `informative_text`: String · `detailed_text`: String · `check_box`: String · `check_box_checked`: Bool · `default_button`: String · `escape_button`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `accepted` · `button_clicked`: String · `details_toggled`: Bool · `focus_changed`: UInt · `rejected`

**Commands.** `set_text`, `set_title`, `set_icon`

**Rust API.** `create_message_box(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("message_box", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `msgbox`

### FileDialog (`file_dialog`)

A modal file-system browser with open, multi-open, save and select-directory modes, navigation history and a sidebar. Use it for any file path the user must choose; `DirectoryDialog` is the directory-mode role of this control, not a separate implementation.

**Properties.** 12 published beyond the four shared: `title`: String · `modal`: Bool · `directory`: String · `selected_file`: String · `mode`: Enum (`open_file` | `open_files` | `save_file` | `select_directory`) · `label_text`: String · `default_suffix`: String · `file_count`: UInt (read-only) · `can_go_back`: Bool (read-only) · `can_go_forward`: Bool (read-only) · `view_mode`: Enum (`list` | `details`) · `sidebar_urls`: StringList — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `accepted` · `current_changed`: String · `directory_entered`: String · `file_selected`: String · `files_selected`: list of String · `focus_changed`: UInt · `overwrite_requested`: String · `quick_look_requested`: String · `rejected` · `selection_changed`: list of String · `sort_changed`: mixed (String parts) · `url_selected`: String

**Commands.** `set_mode`, `set_directory`, `open`

**Rust API.** `create_file_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("file_dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ColorDialog (`color_dialog`)

A modal colour chooser with a wheel or rail, presets, hex entry and an optional screen picker. Use it for a full colour decision; use the inline `ColorPicker` when the picker sits inside a form, and `ColorWell` for a one-swatch collapsed field.

**Properties.** 10 published beyond the four shared: `current_color`: String (read-only) · `hex_rgba`: String · `preset_index`: UInt · `show_hex_input`: Bool · `show_presets`: Bool · `show_hue_rail`: Bool · `custom_color_count`: UInt (read-only) · `modal`: Bool · `options_alpha`: Bool · `show_screen_picker`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `accepted` · `color_selected`: Color · `current_color_changed`: Color · `focus_changed`: UInt · `rejected` · `screen_color_sampled`: Color · `screen_pick_canceled` · `screen_pick_started`

**Commands.** `set_hex`, `apply_preset`

**Rust API.** `create_color_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("color_dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `colour_dialog`

### FontDialog (`font_dialog`)

A modal font chooser with family, style and size selection, a preview and accept/reject. Use it when the user must pick a font and see a specimen; use `FontComboBox` for an inline family field and `FontPreview` for a specimen beside it.

**Properties.** 5 published beyond the four shared: `modal`: Bool · `underline`: Bool · `strikethrough`: Bool · `no_buttons`: Bool · `show_preview`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `accepted` · `font_selected`: String · `rejected`

**Commands.** `set_current_font`, `accept`, `reject`

**Rust API.** `create_font_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("font_dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

### InputDialog (`input_dialog`)

A modal single-value prompt in text, integer, double or item mode, with the matching value property and change signals. Use it for one quick value without building a form; use a `Dialog` with real fields when several values or validation are involved.

**Properties.** 9 published beyond the four shared: `title`: String · `label_text`: String · `mode`: Enum (`text` | `integer` | `double` | `item`) · `text_value`: String · `int_value`: Int · `double_value`: Float · `int_step`: Int · `current_item`: UInt · `dropdown_open`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `accepted` · `double_value_changed`: Float · `focus_changed`: UInt · `int_value_changed`: Int · `rejected` · `text_value_changed`: String

**Commands.** `set_mode`, `set_text_value`, `set_int_value`, `set_double_value`

**Rust API.** `create_input_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("input_dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ProgressDialog (`progress_dialog`)

A modal progress surface with a label, a bar, a range, an optional cancel button and an auto-reset. Use it for cancellable work the user started; use `SplashScreen` for startup work the program owns, and a plain `ProgressBar` when the work is inline and non-blocking.

**Properties.** 11 published beyond the four shared: `title`: String · `label_text`: String · `value`: Int · `minimum`: Int · `maximum`: Int · `auto_reset`: Bool · `show_cancel_button`: Bool · `show_bar`: Bool · `minimum_duration`: UInt · `animated`: Bool · `accent_fill`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `canceled` · `finished` · `label_changed`: String · `value_changed`: Int

**Commands.** `set_value`, `set_range`, `set_title`, `set_label_text`

**Rust API.** `create_progress_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("progress_dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

### PopupWindow (`popup_window`)

A borderless, optionally transient floating window with its own chrome flag, title, content and resize signal. Use it for chrome-only floating content that is not a full dialog; prefer `Popover` for anchored bubbles and `Dialog` when modal semantics are required.

**Properties.** 6 published beyond the four shared: `title`: String · `has_content`: Bool (read-only) · `modal`: Bool · `transient_parent`: UInt · `floating_chrome`: Bool · `resizable`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `closed` · `opened` · `resized`: UInt ×2

**Commands.** `set_content_widget`

**Rust API.** `create_popup_window(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("popup_window", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `popup`

### BottomSheet (`bottom_sheet`)

A panel that slides up from the bottom edge and can be dragged back down, with a peek height and an expanded state. Use it for secondary content on touch layouts; use `ModalBottomSheet` when the sheet must block the page, and `Popover` for a small anchored bubble.

**Properties.** 2 published beyond the four shared: `expanded`: Bool · `peek_height`: Float — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `dismissed`

**Commands.** `set_expanded`, `set_peek_height`

**Rust API.** `create_bottom_sheet(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("bottom_sheet", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `sheet`

### ModalBottomSheet (`modal_bottom_sheet`)

A Material-style modal bottom sheet with a configurable height, an animated presentation and a drag-to-dismiss `dismissed` signal. Use it for a blocking set of choices or details on a phone; use `BottomSheet` for non-blocking peek content and `Dialog` when the decision is a short message.

**Properties.** 2 published beyond the four shared: `sheet_height`: UInt · `animated`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `dismissed`

**Commands.** `set_visible`, `show`, `dismiss`

**Rust API.** `create_modal_bottom_sheet(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("modal_bottom_sheet", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `modal_sheet`

### Tooltip (`tooltip`)

A small popup label shown on hover for context information, with wrap and shown state. Use it for a hint about a control that already has a visible label; do not hide essential instructions in a tooltip, and use the `TooltipManager` companion when the delay and placement must be shared by many controls.

**Properties.** 3 published beyond the four shared: `text`: String · `shown`: Bool · `wrap`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** `set_text`, `show`, `hide`

**Rust API.** `create_tooltip(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tooltip", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `tooltip_widget`, `hover_tip`

### Popover (`popover`)

A floating bubble card with an anchor arrow, a transient/semi-transient/permanent behaviour and a shown state. Use it for a small piece of contextual content near a control; use `Tooltip` for text-only hints and `PopupWindow` when the content needs window chrome.

**Properties.** 6 published beyond the four shared: `shown`: Bool · `text`: String (read-only) · `behavior`: Enum (`transient` | `semi_transient` | `permanent`) · `arrow_direction`: Enum (`auto` | `up` | `down` | `left` | `right`) · `placeholder_text`: String · `translucent`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `about_to_hide` · `about_to_show` · `dismissed` · `shown`

**Commands.** `set_visible`, `set_text`

**Rust API.** `create_popover(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("popover", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `popover_widget`, `popup_card`

### FindReplaceDialog (`find_replace_dialog`)

A find/replace dialog with query and replacement fields, case, whole-word and regex toggles, highlight-all and wrap-around, and next/previous/replace/replace-all signals. Use it for a document-wide search that needs its own modal; use `FindBar` for an inline bar inside an editor.

**Properties.** 7 published beyond the four shared: `find_text`: String · `replace_text`: String · `match_case`: Bool · `whole_word`: Bool · `use_regex`: Bool · `highlight_all`: Bool · `wrap_around`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `close` · `find_next`: String · `find_previous`: String · `focus_changed`: UInt · `replace`: String · `replace_all`: String ×3

**Commands.** `set_find_text`, `set_replace_text`, `set_match_case`, `set_wrap_around`

**Rust API.** `create_find_replace_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("find_replace_dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `find_replace`

### WizardDialog (`wizard_dialog`)

A multi-page dialog that walks the user through ordered steps, with back/next/finish, per-step validity and sidebar pages. Use it for a setup flow where order matters; use a `StackedWidget` plus your own buttons when the steps are not a modal flow, and `Dialog` for a single page.

**Properties.** 6 published beyond the four shared: `title`: String · `subtitle`: String · `current_step`: UInt (read-only) · `step_count`: UInt (read-only) · `can_go_back`: Bool (read-only) · `can_go_forward`: Bool (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `canceled` · `custom_button_clicked`: UInt · `finished` · `help_requested` · `sidebar_page_activated`: UInt · `step_changed`: UInt

**Commands.** `next`, `back`, `finish`

**Rust API.** `create_wizard_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("wizard_dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `wizard`

### AlertDialog (`alert_dialog`)

A modal notice with a message, an icon and one acknowledging action, plus optional informative/detailed text and a check box. Use it for information the user must acknowledge; use `ConfirmDialog` when there is a real accept/cancel choice and `MessageBox` for the classic fixed-button form.

**Properties.** 10 published beyond the four shared: `title`: String · `message`: String · `button_text`: String · `informative_text`: String · `detailed_text`: String · `modal`: Bool · `default_button`: UInt · `check_box`: String · `check_box_checked`: Bool · `icon`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `accepted` · `focus_changed`: UInt · `rejected`

**Commands.** No commands are published.

**Rust API.** `create_alert_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("alert_dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `notice_dialog`

### ConfirmDialog (`confirm_dialog`)

A modal question with an accept action, a cancel action, optional informative/detailed text and a check box. Use it before a destructive or irreversible operation; use `AlertDialog` for information only and `MessageBox` when the buttons must be the classic set.

**Properties.** 11 published beyond the four shared: `title`: String · `message`: String · `accept_text`: String · `reject_text`: String · `informative_text`: String · `detailed_text`: String · `modal`: Bool · `default_button`: UInt · `check_box`: String · `check_box_checked`: Bool · `icon`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `accepted` · `focus_changed`: UInt · `rejected`

**Commands.** No commands are published.

**Rust API.** `create_confirm_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("confirm_dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `confirmation_dialog`

### AboutDialog (`about_dialog`)

The standard application-information dialog: application name, version, message, informative and detailed text, plus an optional check box. Use it for Help/About; do not use it as a general-purpose dialog — those fields are the whole contract.

**Properties.** 11 published beyond the four shared: `title`: String · `application_name`: String · `version`: String · `message`: String · `informative_text`: String · `detailed_text`: String · `modal`: Bool · `default_button`: UInt · `check_box`: String · `check_box_checked`: Bool · `icon`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `closed`

**Commands.** No commands are published.

**Rust API.** `create_about_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("about_dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `about_box`

## 11. Composite application surfaces

The special family holds the controls that are whole application surfaces rather than a single field: the drawing canvas, map, generic chart, Gantt and timeline, kanban board, colour and shape pickers, code-adjacent viewers (diff, terminal, markdown), notification/toast surfaces, and the companion controls those surfaces are paired with.

Choose a special control when the feature *is* the surface — a canvas for free drawing, `MapView` for tiled geography, `KanbanBoard` for two-level drag-and-drop, `DiffViewer` for a side-by-side or unified comparison. Each of these owns a model and exposes a narrow signal set; the host feeds data in and reacts to signals. Do not use them as generic containers, and do not try to reproduce their interaction with a grid of small controls: the drag, hit-testing and viewport rules are the control's value.

| Family | Source directory | Controls |
|---|---|---|
| `special` | `src/widget/special_widgets/` | 39 |

### Canvas (`canvas`)

A free-form drawing surface with layers, zoom, pan, grid snapping and pointer events carrying positions. Use it for a scene the host draws itself; use `ShapeCanvas` when the scene is a set of selectable shapes with an editor, and `MiniCanvas` for a small immediate-mode decoration.

**Properties.** 8 published beyond the four shared: `command_count`: UInt (read-only) · `layer_count`: UInt (read-only) · `anti_alias`: Bool · `show_grid`: Bool · `grid_step`: UInt · `snap_to_grid`: Bool · `zoom_range`: NumberList · `zoom_step`: Float — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `command_picked`: UInt · `content_changed`: UInt · `context_menu_requested`: String · `double_clicked`: mixed (String parts) · `layer_changed`: UInt · `mouse_moved`: String · `mouse_pressed`: mixed (String parts) · `mouse_released`: mixed (String parts) · `zoom_changed`: Float

**Commands.** `set_zoom`, `set_center`, `clear`

**Rust API.** `create_canvas(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("canvas", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `canvas_widget`, `drawing_surface`

### Canvas (`map_view`)

A tiled map view with a centre, zoom, projection and markers, owning its `TileLoader`, `ZoomControl`, `ScaleBar` and `Compass` companions. The host fetches tiles and calls `deliver_tile`/`reject_tile`; the view requests them through `tile_requested`, so no I/O happens in the widget.

**Properties.** 6 published beyond the four shared: `center_x`: Float · `center_y`: Float · `zoom`: Float · `marker_count`: UInt (read-only) · `selected_marker_id`: String (read-only) · `projection`: Enum (`linear` | `mercator`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `center_changed`: Float ×2 · `cluster_clicked`: Float ×2 · `marker_clicked`: String · `marker_selected`: String · `tile_requested`: mixed (String parts) · `zoom_changed`: Float

**Commands.** `set_markers`, `set_center_x`, `set_center_y`, `set_zoom`

**Rust API.** `create_map_view(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("map_view", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Chart (`chart`)

The general chart widget: a chart type (bar, line, area, pie, scatter, waterfall, funnel, candlestick, box plot), a data series list, labels, zoom and hover readout. Use it when the type is chosen at runtime; use the dedicated `LineChart`/`BarChart`/`PieChart` controls for a fixed type, and the finance family for market data.

**Properties.** 4 published beyond the four shared: `chart_type`: Enum (`bar` | `line` | `area` | `pie` | `scatter` | `waterfall` | `funnel` | `candlestick` | `box_plot`) · `point_count`: UInt (read-only) · `label_count`: UInt (read-only) · `series_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `data_point_clicked`: UInt · `data_point_hovered`: UInt · `data_point_unhovered`: UInt · `viewport_changed`: UInt ×2 · `zoom_changed`: (Float, Float) or null

**Commands.** No commands are published.

**Rust API.** `create_chart(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("chart", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `chart_widget`, `chart_surface`

### Chart (`gantt_widget`)

A Gantt chart of tasks against a viewport, with milestones, dependencies, critical path, progress resizing and a today marker; it owns a `TimelineAxis` and `DependencyEditor`. Use it for project scheduling; use `TimelineWidget` for a non-scheduling event lane view.

**Properties.** 7 published beyond the four shared: `task_count`: UInt (read-only) · `selected_id`: String (read-only) · `viewport_start`: Int · `viewport_end`: Int · `row_height`: UInt · `show_critical_path`: Bool · `show_today`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `dependency_added`: String ×2 · `dependency_removed`: String ×2 · `link_selected`: String ×2 · `task_rescheduled`: String · `task_resize_finished`: String · `task_resize_started`: String · `task_selected`: String · `viewport_changed`: Int ×2

**Commands.** `set_tasks`, `zoom`, `set_viewport`, `select_task`

**Rust API.** `create_gantt_widget(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("gantt_widget", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `gantt`

### Chart (`timeline_widget`)

A timeline of items placed on a time axis, with row or lane-packed layout and a selectable item. Use it for event histories and activity feeds; use `GanttWidget` when tasks have duration, dependencies and progress.

**Properties.** 7 published beyond the four shared: `item_count`: UInt (read-only) · `selected_index`: UInt (read-only) · `viewport_start`: Int · `viewport_end`: Int · `row_height`: UInt · `show_time_labels`: Bool · `lane_mode`: Enum (`row` | `lane_packed`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `item_hovered`: String or null · `item_selected`: String · `viewport_changed`: Int ×2

**Commands.** `set_items`, `set_viewport`, `zoom`, `select_index`

**Rust API.** `create_timeline_widget(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("timeline_widget", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `timeline`, `timeline_view`

### ColorPicker (`color_picker`)

An inline HSVA colour picker with hex entry, alpha, presets and a hue value; it is embedded in a form rather than hosted in a window. Use it when the colour field is always visible; use `ColorDialog` for a modal picker and `ColorWell` for the collapsed single-swatch field.

**Properties.** 5 published beyond the four shared: `hex_rgba`: String · `show_alpha`: Bool · `preset_count`: UInt (read-only) · `presets`: StringList · `hue_degrees`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `canceled` · `color_changed`: Color · `color_committed`: Color · `color_previewed`: Color · `hex_changed`: String

**Commands.** `set_hex`, `apply_preset`

**Rust API.** `create_color_picker(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("color_picker", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Grid (`grid`)

A rectangular container that arranges children into rows and columns of cells, with spacing, cell size, headers and paging. Use it for a fixed matrix of controls or values; use `Table` when cells come from a model and `Flex`/`Grid` layout names for pure placement.

**Properties.** 11 published beyond the four shared: `rows`: UInt · `columns`: UInt · `spacing`: UInt · `line_color`: String · `cell_width`: UInt (read-only) · `cell_height`: UInt (read-only) · `selected_cell`: UInt · `selected_count`: UInt (read-only) · `page`: UInt · `page_count`: UInt (read-only) · `headers`: StringList — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `cell_clicked`: UInt ×2 · `cell_hovered`: UInt ×2 · `context_menu_requested`: UInt ×2 · `page_changed`: UInt

**Commands.** `set_rows`, `set_columns`, `set_spacing`, `set_line_color`

**Rust API.** `create_grid(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("grid", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `grid_widget`, `gridwidget`

### FreeformShape (`freeform_shape`)

A path-based, non-rectangular clickable shape with fill, stroke, vertices, history and move/vertex signals. Use it for a diagram node or a custom hit region; use `ShapeCanvas` when several shapes form an editable scene, and `MiniCanvas` for non-interactive drawing.

**Properties.** 5 published beyond the four shared: `path_kind`: Enum (`bubble` | `rounded_rect` | `polygon` | `star` | `heart` | `custom`) (read-only) · `fill_rgba`: String · `stroke_rgba`: String · `stroke_width`: UInt · `movable`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `clicked` · `geometry_changed`: Rect · `history_changed`: UInt ×2 · `hover_changed`: Bool · `moved`: String · `pressed_changed`: Bool · `vertex_changed`: mixed (String parts)

**Commands.** `set_fill_rgba`, `set_stroke_rgba`, `set_stroke_width`

**Rust API.** `create_freeform_shape(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("freeform_shape", geometry, text)` and `WidgetFactory::shared()` for the registry.

### RadarChart (`radar_chart`)

A radar (spider) chart with shared dimension axes, several series drawn as closed polygons, grid, axis labels and legend. Use it for multi-axis profiles (skills, KPIs) where the dimensions are comparable; use `LineChart` for one ordered axis and `Heatmap` when a matrix is the data.

**Properties.** 7 published beyond the four shared: `axis_count`: UInt (read-only) · `series_count`: UInt (read-only) · `show_grid`: Bool · `show_axis_labels`: Bool · `show_legend`: Bool · `hovered_axis`: UInt (read-only) · `series_names`: StringList — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `axis_hovered`: UInt · `series_clicked`: UInt · `vertex_clicked`: UInt ×2 · `vertex_hovered`: UInt ×2

**Commands.** `set_axes`, `set_series`, `add_series`

**Rust API.** `create_radar_chart(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("radar_chart", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `radar`, `spider_chart`

### Heatmap (`heatmap`)

A matrix of values rendered as coloured cells, with a scale range, labels, legend and a viewport window. Use it for a density or correlation matrix where both axes are categorical; use `Grid` for plain cells and `Chart` for a value-per-index series.

**Properties.** 10 published beyond the four shared: `row_count`: UInt (read-only) · `column_count`: UInt (read-only) · `scale_minimum`: Float · `scale_maximum`: Float · `resolved_minimum`: Float (read-only) · `resolved_maximum`: Float (read-only) · `show_labels`: Bool · `show_legend`: Bool · `viewport_rows`: UInt · `viewport_columns`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `cell_clicked`: UInt ×2 · `cell_double_clicked`: UInt ×2 · `cell_hovered`: UInt ×2 · `region_selected`: UInt ×4

**Commands.** `set_data`, `set_cell`, `clear`

**Rust API.** `create_heatmap(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("heatmap", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `heatmap_chart`

### KanbanBoard (`kanban_board`)

A board of columns holding draggable cards, with lanes, column width, horizontal offset and card-moved/activated signals. Pair it with `KanbanColumnHeader` for rename/count/WIP chrome and `KanbanCardView` when a card needs a real editable control; the board owns the model and the drop rules.

**Properties.** 9 published beyond the four shared: `column_count`: UInt (read-only) · `card_count`: UInt (read-only) · `dragging_card_id`: String (read-only) · `hovered_column`: UInt (read-only) · `column_width`: UInt · `horizontal_scroll_offset`: UInt · `lane_mode`: Enum (`none` | `lanes`) · `swimlanes`: Bool · `lane_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `card_activated`: String · `card_moved`: String ×2

**Commands.** `add_column`, `add_card`, `move_card`, `cancel_drag`

**Rust API.** `create_kanban_board(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("kanban_board", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `kanban`, `board`

### SignaturePad (`signature_pad`)

A touch-friendly freehand drawing surface for signatures: strokes with width, colour, smoothing thresholds and pressure, plus undo and a stroke-completed signal. Use it for a signature or a scribble input; do not use it as a general paint program — it stores polylines, not a bitmap.

**Properties.** 9 published beyond the four shared: `stroke_count`: UInt (read-only) · `stroke_width`: UInt · `stroke_color`: Color · `min_point_distance`: Float · `min_point_interval_ms`: UInt · `pressure_affects_width`: Bool · `last_pressure`: Float (read-only) · `last_tilt_x`: Float (read-only) · `last_tilt_y`: Float (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `changed` · `stroke_completed`: String

**Commands.** `undo`

**Rust API.** `create_signature_pad(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("signature_pad", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `signature`

### Breadcrumb (`breadcrumb`)

A horizontal navigation trail of segments, with collapse, overflow, inline path editing and per-segment activation. Use it for a hierarchy the user can jump back up; use `Pagination` for numbered pages and `MenuBar` for top-level commands.

**Properties.** 4 published beyond the four shared: `segment_count`: UInt (read-only) · `selected_index`: UInt (read-only) · `collapsed_count`: UInt (read-only) · `separator`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `context_menu_requested`: String · `overflow_clicked`: UInt · `path_edited`: String · `segment_activated`: String

**Commands.** `set_segments`, `push_segment`, `clear_segments`

**Rust API.** `create_breadcrumb(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("breadcrumb", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `nav_breadcrumb`

### Chip (`chip`)

A multi-item pill row: chips can be selected, removed, reordered and overflowed, with a focus index. Use it for a token/tag set that is edited as a group; use a single `Tag` for one read-only label and `TagInput` when the user types new values.

**Properties.** 4 published beyond the four shared: `item_count`: UInt (read-only) · `multi_select`: Bool · `focused_index`: UInt (read-only) · `selected_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `chip_removed`: String · `chip_reordered`: String · `chip_toggled`: String · `overflow_activated`: UInt

**Commands.** `set_items`, `toggle_index`, `move_focus`

**Rust API.** `create_chip(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("chip", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `chips`

### Toast (`toast`)

One transient notification message with a level, a time-to-live and a dismiss signal. Create it through the toast surface or the `ToastHost` queue rather than managing many standalone toasts; use `Banner` when the notice must persist and `Snackbar` when it belongs to a window edge.

**Properties.** 4 published beyond the four shared: `message`: String · `level`: Enum (`info` | `success` | `warning` | `error`) · `ttl_ms`: UInt · `dismissible`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `dismissed`: String

**Commands.** `dismiss`, `set_message`, `set_level`

**Rust API.** `create_toast(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("toast", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `notification`, `toast_message`

### PopupWindow (`toast_stack`)

The container that queues and lays out toasts, with a visible count, max visible, anchor and stacking direction. Use it as the single owner of toast placement; pair it with `ToastHost` when the queue must expire and cap entries before they reach the stack.

**Properties.** 6 published beyond the four shared: `toast_count`: UInt (read-only) · `selected_id`: String (read-only) · `row_height`: UInt · `max_visible`: UInt · `anchor`: Enum (`top_left` | `top_right` | `bottom_left` | `bottom_right`) · `stacking_direction`: Enum (`newest_at_bottom` | `newest_at_top`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `toast_action`: String · `toast_activated`: String · `toast_dismissed`: String · `toast_expired`: String

**Commands.** `push`, `clear`, `select_index`, `activate_selected`, `dismiss_selected`

**Rust API.** `create_toast_stack(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("toast_stack", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `toasts`

### ListView (`command_palette`)

A searchable command list overlay: query, filtered count, highlighted index and `command_activated`. Use it for a keyboard-first launcher; use `Menu` for a mouse-driven command list and `ShortcutEditor` to edit bindings.

**Properties.** 6 published beyond the four shared: `query`: String · `entry_count`: UInt (read-only) · `filtered_count`: UInt (read-only) · `highlighted_index`: UInt (read-only) · `row_height`: UInt · `group_by_category`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `command_activated`: String · `query_changed`: String

**Commands.** `set_entries`, `set_query`, `clear_query`, `move_highlight`, `activate_highlighted`

**Rust API.** `create_command_palette(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("command_palette", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `command_box`

### ListView (`notification_center`)

A list of notifications with unread count, level filter, max items and activate/dismiss/clear-all signals. Use it for a persistent inbox of events; use `Toast`/`ToastStack` for transient interruptions and `Banner` for one persistent notice.

**Properties.** 6 published beyond the four shared: `item_count`: UInt (read-only) · `unread_count`: UInt (read-only) · `selected_index`: UInt (read-only) · `row_height`: UInt · `level_filter`: Enum (`all` | `info` | `warning` | `error`) · `max_items`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `clear_all_requested` · `notification_activated`: String · `notification_dismissed`: String · `notification_selected`: String · `unread_count_changed`: UInt

**Commands.** `push`, `clear`, `set_read`, `mark_all_read`, `select_index`, `activate_selected`

**Rust API.** `create_notification_center(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("notification_center", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `notifications`

### Table (`diff_viewer`)

A side-by-side or unified diff view over two texts, with change count, fold thresholds, whitespace modes, word granularity and hunk expansion. Use it to show a comparison; pair it with `DiffFileList` to choose among several files, and do not use it as an editor — it is read-only.

**Properties.** 12 published beyond the four shared: `left_text`: String · `right_text`: String · `line_count`: UInt (read-only) · `change_count`: UInt (read-only) · `selected_index`: UInt (read-only) · `view_mode`: Enum (`side_by_side` | `unified`) · `fold_unchanged`: Bool · `fold_threshold`: UInt · `fold_context_lines`: UInt · `unified_changed_pair`: Bool · `whitespace_mode`: Enum (`show` | `ignore_eol` | `ignore_all` | `mark_blank_changes`) · `word_granularity`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `compared`: UInt · `fold_changed`: Bool · `hunk_expanded`: UInt · `selection_changed`: UInt or null · `view_mode_changed`: String

**Commands.** `set_texts`, `select_index`

**Rust API.** `create_diff_viewer(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("diff_viewer", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `diff`

### SegmentedControl (`segmented_control`)

A single-select segmented bar whose indicator slides between segments, with an id per segment. Use it for two to five mutually exclusive views; use `SegmentedButton` for multi-select or icon segments and `TabBar` when the segments are page tabs.

**Properties.** 6 published beyond the four shared: `item_count`: UInt (read-only) · `selected_index`: UInt (read-only) · `selected_id`: String (read-only) · `size_to_fit`: Bool · `min_segment_width`: UInt · `multi_select`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `selection_changed`: String

**Commands.** `set_items`, `move_selection`

**Rust API.** `create_segmented_control(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("segmented_control", geometry, text)` and `WidgetFactory::shared()` for the registry.

### StatusBar (`snackbar`)

A transient message anchored to a window edge, with an action label, a queue length and a max queue. Use it to confirm a completed action without blocking; use `Banner` when the message must persist and `ToastStack` when several messages must stack.

**Properties.** 5 published beyond the four shared: `message`: String · `action_label`: String (read-only) · `queued_count`: UInt (read-only) · `max_queued`: UInt · `anchor`: Enum (`top` | `center` | `bottom`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `action_triggered`: String · `dismissed`

**Commands.** `show`, `show_with_action`, `dismiss`

**Rust API.** `create_snackbar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("snackbar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `toast_bar`

### WebEngineView (`media_player`)

A media surface with a source, play/pause, position, duration, volume and fullscreen, reporting playback and position changes. Use it when the host supplies the media pipeline; pair it with the `MediaTransportBar`, `MediaScrubber` and `SubtitleBar` companions rather than re-implementing controls.

**Properties.** 9 published beyond the four shared: `source`: String · `playing`: Bool · `duration_ms`: UInt (read-only) · `position_ms`: UInt · `volume`: UInt · `muted`: Bool · `fullscreen`: Bool · `playlist_index`: UInt · `track_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `chapter_selected`: UInt · `fullscreen_requested`: Bool · `media_ended` · `playback_changed`: Bool · `position_changed`: UInt · `source_changed`: String · `volume_changed`: UInt

**Commands.** `set_source`, `clear_source`, `play`, `pause`, `seek_to`, `set_volume`

**Rust API.** `create_media_player(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("media_player", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ShapeCanvas (`shape_canvas`)

A scene that hosts, selects and orders shapes, with add/modify/remove/select signals, snapping and a grid step. Use it for a diagram editor; use `FreeformShape` for one shape and `Canvas` for arbitrary drawing not built from shape objects.

**Properties.** 6 published beyond the four shared: `shapes`: NumberList (read-only) · `shape_count`: UInt (read-only) · `selection`: NumberList · `editable`: Bool · `snap_to_grid`: Bool · `grid_step`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `shape_added`: UInt · `shape_modified`: UInt · `shape_removed`: UInt · `shape_selected`: list of UInt

**Commands.** No commands are published.

**Rust API.** `create_shape_canvas(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("shape_canvas", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `shape_board`

### KanbanCardView (`kanban_card_view`)

A real card control for a kanban column: title, description, tags, assignee, done state and in-place editing. Use it when a card must be an actual editable control; the `KanbanBoard` owns placement and drag state, and this control owns one card's content and edit lifecycle.

**Properties.** 7 published beyond the four shared: `title`: String · `description`: String · `done`: Bool · `tags`: StringList · `assignee`: String · `draggable`: Bool · `editing`: Bool (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `card_clicked` · `card_done_toggled`: Bool · `card_edited`: String

**Commands.** No commands are published.

**Rust API.** `create_kanban_card_view(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("kanban_card_view", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `kanban_card_control`

### KanbanColumnHeader (`kanban_column_header`)

A real column header for a kanban board: title, card count, WIP limit, collapse and inline rename. Use it above a `KanbanBoard` column; the header owns its own rename and collapse state and reports `column_renamed`, `column_collapsed_changed` and `add_card_requested`.

**Properties.** 7 published beyond the four shared: `title`: String · `count`: UInt · `wip_limit`: UInt · `collapsed`: Bool · `editable`: Bool · `at_wip_limit`: Bool (read-only) · `editing`: Bool (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `add_card_requested` · `column_collapsed_changed`: Bool · `column_renamed`: String

**Commands.** No commands are published.

**Rust API.** `create_kanban_column_header(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("kanban_column_header", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `kanban_column_bar`

### DiffFileList (`diff_file_list`)

The multi-file selector beside a diff viewer: a list of paths with a selection mode and activate/select signals. The view owns the comparison; this control only chooses which path the view should show.

**Properties.** 4 published beyond the four shared: `files`: StringList · `file_count`: UInt (read-only) · `selected_path`: String (read-only) · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `file_activated`: String · `file_selected`: String

**Commands.** No commands are published.

**Rust API.** `create_diff_file_list(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("diff_file_list", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `diff_files`

### Swatch (`swatch`)

A selectable grid of colour cells shared by the colour controls, with a selected index and activated/selected signals. The dialog or picker owns the current colour; the swatch owns the palette and the selection, so wire `swatch_selected` into the owner's colour property.

**Properties.** 4 published beyond the four shared: `colors`: Color · `selected_index`: UInt · `cell_size`: UInt · `columns`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `swatch_activated`: Color · `swatch_selected`: UInt

**Commands.** No commands are published.

**Rust API.** `create_swatch(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("swatch", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `color_swatch`, `colour_swatch`

### FilterRow (`filter_row`)

One editor per column that publishes the row's filter conditions and a live flag. Use it beside a table for a compact multi-column filter; the table or grid owns the model, and the row only produces conditions.

**Properties.** 4 published beyond the four shared: `columns`: StringList · `conditions`: StringList · `operator`: Enum (`contains` | `equals` | `starts_with` | `ends_with` | `not_contains` | `equals_number` | `greater_than` | `greater_or_equal` | `less_than` | `less_or_equal`) · `live`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `filter_changed`: String · `filter_cleared`

**Commands.** No commands are published.

**Rust API.** `create_filter_row(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("filter_row", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `filter_bar`

### Legend (`legend`)

A checkable series key for charts and canvas hosts: entries, orientation, position and click/toggle signals. The chart owns the series data; the legend owns which series are visible, so forward `series_toggled` to the chart's visibility state.

**Properties.** 4 published beyond the four shared: `entries`: StringList · `orientation`: Enum (`horizontal` | `vertical`) · `checkable`: Bool · `position`: Enum (`top` | `right` | `bottom` | `left` | `outside`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `entry_clicked`: UInt · `series_toggled`: mixed (String parts)

**Commands.** No commands are published.

**Rust API.** `create_legend(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("legend", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `chart_legend`

### ChartTooltip (`chart_tooltip`)

A follow-pointer bubble carrying chart series values, with an anchor, an offset, a follow flag and content lines. The chart positions it through `show_at`; the tooltip owns only its text and visibility.

**Properties.** 4 published beyond the four shared: `anchor`: NumberList · `offset`: NumberList · `follow_pointer`: Bool · `content`: StringList — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `hidden` · `shown`

**Commands.** No commands are published.

**Rust API.** `create_chart_tooltip(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("chart_tooltip", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `tooltip_bubble`

### TooltipManager (`tooltip_manager`)

The host pipeline that turns hover into a delayed tooltip: delay, duration, follow-pointer and max width, with tooltip-shown/hidden indices. Use one manager for a subtree when many controls share tooltip timing; individual controls can still use their own `tooltip` property.

**Properties.** 4 published beyond the four shared: `delay_ms`: UInt · `duration_ms`: UInt · `follow_pointer`: Bool · `max_width`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `tooltip_hidden`: UInt · `tooltip_shown`: UInt

**Commands.** No commands are published.

**Rust API.** `create_tooltip_manager(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tooltip_manager", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `tooltip_pipeline`

### ZoomPanController (`zoom_pan_controller`)

Zoom, pan and region-of-interest state for an image view, with a zoom mode, limits and change signals. Use it when several controls must share the same view transform; the image view paints, and the controller owns the transform.

**Properties.** 6 published beyond the four shared: `zoom_level`: Float · `pan_offset`: NumberList · `zoom_mode`: Enum (`free` | `fit_width` | `fit_height` | `original`) · `min_zoom`: Float · `max_zoom`: Float · `roi`: Rect — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `pan_changed`: Int ×2 · `roi_changed`: Rect or null · `zoom_changed`: Float

**Commands.** No commands are published.

**Rust API.** `create_zoom_pan_controller(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("zoom_pan_controller", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `zoom_pan`

### ToastHost (`toast_host`)

The bounded, expiring queue in front of a toast stack: max visible, max queued, default duration, anchor, stacking direction and an overflow policy. Use it when toast volume must be capped; the stack paints, and the host decides which toasts live long enough to reach it.

**Properties.** 6 published beyond the four shared: `max_visible`: UInt · `max_queued`: UInt · `default_duration_ms`: UInt · `anchor`: Enum (`top_left` | `top_center` | `top_right` | `bottom_left` | `bottom_center` | `bottom_right`) · `stacking_direction`: Enum (`newest_at_top` | `newest_at_bottom`) · `overflow_policy`: Enum (`drop_oldest` | `drop_newest` | `queue`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `queue_overflow`: UInt · `toast_dismissed`: UInt · `toast_expired`: UInt

**Commands.** No commands are published.

**Rust API.** `create_toast_host(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("toast_host", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `toast_queue`

### TileLoader (`tile_loader`)

Tile addressing, caching and load results for a map view: a URL template, zoom range, tile size, cache limit and tile-loaded/failed signals. The map view owns one and asks it for URLs; the host performs the request and calls `deliver_tile`.

**Properties.** 6 published beyond the four shared: `url_template`: String · `min_zoom`: UInt · `max_zoom`: UInt · `tile_size`: UInt · `cache_limit`: UInt · `cache_size`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `tile_failed`: Int ×3 · `tile_loaded`: Int ×3

**Commands.** `clear_cache`

**Rust API.** `create_tile_loader(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tile_loader", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `map_tile_loader`

### ZoomControl (`zoom_control`)

The three-button zoom strip beside a map: zoom level, min/max and a `zoom_requested` signal. The map owns the strip and applies the request; do not let the strip mutate the map's zoom directly if the map also reports zoom changes.

**Properties.** 3 published beyond the four shared: `zoom_level`: Float · `min`: Float · `max`: Float — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `zoom_requested`: Float

**Commands.** No commands are published.

**Rust API.** `create_zoom_control(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("zoom_control", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `map_zoom_control`

### ScaleBar (`scale_bar`)

The metric or imperial distance legend for a map, computed from metres per pixel. The map owns it and updates the conversion; the bar only paints the legend.

**Properties.** 3 published beyond the four shared: `meters_per_pixel`: Float · `units`: Enum (`metric` | `imperial`) · `style`: Enum (`plain` | `boxed`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** No commands are published.

**Rust API.** `create_scale_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("scale_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `map_scale_bar`

### Compass (`compass`)

A clickable bearing dial that requests north-up when clicked, reporting its bearing. Use it over a rotatable map; the map owns the bearing and answers `reset_north_requested`.

**Properties.** 2 published beyond the four shared: `bearing_deg`: Float · `clickable`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `reset_north_requested`

**Commands.** No commands are published.

**Rust API.** `create_compass(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("compass", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `map_compass`

### TimelineAxis (`timeline_axis`)

A date/value axis with pixel conversion, tick density, zoom and scale-change reporting. The Gantt chart owns one and uses `pixel_to_date`/`date_to_pixel`; a host may place another beside a chart to share the same mapping.

**Properties.** 6 published beyond the four shared: `tick_density`: UInt · `date_format`: Enum (`numeric` | `iso`) · `zoom_level`: Float · `range`: NumberList (read-only) · `range_start`: Int · `range_end`: Int — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `scale_changed`: Int ×2

**Commands.** No commands are published.

**Rust API.** `create_timeline_axis(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("timeline_axis", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `gantt_timeline_axis`

### DependencyEditor (`dependency_editor`)

Editable predecessor/successor edges for a Gantt chart, with add/remove/select signals, hit testing and link geometry. The Gantt owns one and can sync dependencies from its tasks; the editor owns the edge list and validation.

**Properties.** 2 published beyond the four shared: `dependencies`: UInt (read-only) · `editable`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `dependency_added`: String ×2 · `dependency_removed`: String ×2 · `link_selected`: String ×2

**Commands.** No commands are published.

**Rust API.** `create_dependency_editor(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("dependency_editor", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `gantt_dependency_editor`

## 12. Code and document editors

This family groups the editors whose content is structured text: the code editor with its own buffer, the markdown editor with preview and headings, and the find bar and outline panel that navigate them.

Choose `CodeEditor` for source text with line numbers, markers, folds and cursor events; `MarkdownEditor` for prose with a preview split; `FindBar` when a host wants its own find UI; `MarkdownOutlinePanel` for heading navigation. Do not use them for single-line values (`LineEdit`) or rich formatted text (`RichEdit`), and do not use `TextArea` when syntax markers or line gutters are required.

| Family | Source directory | Controls |
|---|---|---|
| `code_editor` | `src/widget/special_widgets/code_editor/` and `src/widget/special_widgets/markdown_editor.rs` | 5 |

### RichEdit (`code_editor`)

A source editor with its own buffer: line count, cursor line and column, markers, folds, a goto bar and a breakpoint gutter, reporting cursor, fold, marker and search events. Use it for source text; use `MarkdownEditor` for prose, `RichEdit` for formatting and `TerminalView` for append-only output.

**Properties.** 9 published beyond the four shared: `text`: String · `line_count`: UInt (read-only) · `cursor_line`: UInt (read-only) · `cursor_column`: UInt (read-only) · `marker_count`: UInt (read-only) · `undo_stack_limit`: UInt · `undo_depth`: UInt (read-only) · `goto_bar_visible`: Bool · `breakpoint_gutter`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `breakpoint_toggled`: mixed (String parts) · `completion_changed`: Bool · `cursor_moved`: UInt ×2 · `fold_changed`: UInt · `marker_activated`: UInt · `palette_changed` · `search_changed`: UInt · `search_result`: UInt ×4 · `selection_changed`: Rect or null · `tab_changed`: UInt · `text_changed`: String · `zoom_changed`: Float

**Commands.** `set_text`, `append_line`, `set_markers`, `set_cursor`

**Rust API.** `create_code_editor(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("code_editor", geometry, text)` and `WidgetFactory::shared()` for the registry.

### RichEdit (`markdown_editor`)

A markdown editor with source and preview modes, heading count, cursor tracking and an outline that a `MarkdownOutlinePanel` can display. Use it for markdown documents; pair it with `FindBar` for inline search and use `RichEdit` for non-markdown formatting.

**Properties.** 10 published beyond the four shared: `text`: String · `preview_mode`: Bool · `line_count`: UInt (read-only) · `word_count`: UInt (read-only) · `heading_count`: UInt (read-only) · `cursor_line`: UInt (read-only) · `show_scrollbar`: Bool · `gfm_extensions`: Bool · `code_highlight`: Bool · `undo_stack_limit`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `heading_activated`: UInt · `preview_mode_changed`: Bool · `selection_changed`: Bool · `text_changed`: String

**Commands.** `set_text`, `append_line`, `toggle_preview_mode`, `set_preview_mode`, `undo`

**Rust API.** `create_markdown_editor(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("markdown_editor", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `md_editor`

### FindBar (`find_bar`)

An inline search bar for an editor: query, case, whole-word, replacement text, match count and the current range, reporting search and replace requests. Use it inside an editor surface; use `FindReplaceDialog` when the search must be modal.

**Properties.** 7 published beyond the four shared: `query`: String · `case_sensitive`: Bool · `whole_word`: Bool · `replace_text`: String · `replace_visible`: Bool · `match_count`: UInt (read-only) · `current_range`: String (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `replace_requested`: mixed (String parts) · `search_changed`: UInt ×2

**Commands.** No commands are published.

**Rust API.** `create_find_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("find_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `find_replace_bar`

### MarkdownOutlinePanel (`markdown_outline_panel`)

A heading outline that navigates a markdown editor: entries, active index and width, emitting `entry_selected`. The editor owns the document and `outline()`; the panel only lists headings and reports the chosen index back through `goto_outline`.

**Properties.** 4 published beyond the four shared: `entries`: StringList · `active_index`: UInt · `width`: UInt · `entry_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `entry_selected`: UInt

**Commands.** No commands are published.

**Rust API.** `create_markdown_outline_panel(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("markdown_outline_panel", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `markdown_outline`

### TextEdit (`terminal_view`)

An append-only terminal surface: scrollback output, an input line, a prompt, caret style, ANSI palette and a `command_submitted` signal. Use it to show a process or a command console; use `TextArea` for editable text and `CodeEditor` for source.

**Properties.** 10 published beyond the four shared: `output_line_count`: UInt (read-only) · `input_line`: String · `scrollback_lines`: UInt · `wrap_output`: Bool · `show_scrollbar`: Bool · `caret_style`: Enum (`line` | `block` | `underline`) · `caret_blink_enabled`: Bool · `prompt`: String · `selection_mode`: Enum (`lines` | `characters`) · `ansi_palette`: NumberList — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `command_submitted`: String · `completion_requested`: String · `interrupt_requested`: String · `output_received`: String · `resized`: UInt ×2 · `selection_changed`: Bool

**Commands.** `append_output`, `submit`

**Rust API.** `create_terminal_view(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("terminal_view", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `terminal`

## 13. Market-data controls

The finance family is a vertically integrated market view: candlesticks with overlays, an aligned volume histogram, a depth curve, an order-book ladder, a quote board and an indicator pane. They share a series/layout vocabulary so panels line up on one price-and-time axis.

Choose a finance control only for market or trading data: `CandlestickChart` for OHLC bars, `VolumeChart` aligned beneath it, `DepthChart` for cumulative liquidity, `OrderBook` for the live ladder, `QuoteBoard` for a watchlist, and `IndicatorChart` for MACD/RSI/KDJ-style panes. Do not use them for generic business charts (the chart family), and do not feed them historical snapshots if live updates are not needed — a static table is cheaper.

| Family | Source directory | Controls |
|---|---|---|
| `finance` | `src/widget/chart_widgets/finance/` | 6 |

### CandlestickChart (`candlestick_chart`)

An OHLC candlestick chart with overlay series, a price axis, hover/click per bar and a view change. Use it as the main price pane of a market view; align `VolumeChart` beneath it and `IndicatorChart` for technical panes. Do not use it for non-OHLC business data.

**Properties.** 4 published beyond the four shared: `series`: String · `overlay_count`: UInt · `show_price_levels`: Bool · `show_overlay_legend`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `bar_clicked`: UInt · `bar_hovered`: UInt · `bar_unhovered`: UInt · `view_changed`: UInt ×2

**Commands.** `set_series`, `add_overlay`

**Rust API.** `create_candlestick_chart(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("candlestick_chart", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `candlestick`, `kline`, `k_line`, `k_line_chart`, `ohlc_chart`

### VolumeChart (`volume_chart`)

A volume histogram aligned to a candlestick chart's bars, with a direction or uniform colour mode and headroom. The price chart owns the time axis; this pane reads the same series and renders volume, so keep the two viewports in sync from the host.

**Properties.** 3 published beyond the four shared: `series`: String · `color_mode`: Enum (`direction` | `uniform`) · `headroom`: Float — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `bar_clicked`: UInt · `bar_hovered`: UInt · `view_changed`: UInt ×2

**Commands.** `set_series`

**Rust API.** `create_volume_chart(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("volume_chart", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `volume`, `volume_histogram`, `volume_bars`

### DepthChart (`depth_chart`)

A cumulative bid/ask depth curve with configurable depth and separate bid/ask colours, reporting the price level under the pointer. Use it for liquidity; use `OrderBook` for the discrete ladder with sizes.

**Properties.** 3 published beyond the four shared: `depth`: UInt · `bid_color`: Color · `ask_color`: Color — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `level_clicked`: Float · `level_hovered`: Float

**Commands.** `set_series`

**Rust API.** `create_depth_chart(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("depth_chart", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `market_depth`, `depth_graph`, `liquidity_chart`

### OrderBook (`order_book`)

A live bid/ask ladder with sizes at each level, a spread row and per-level hover/click. Use it beside a candlestick chart for microstructure; use `DepthChart` for the smooth cumulative curve and `QuoteBoard` for a multi-instrument watchlist.

**Properties.** 3 published beyond the four shared: `depth`: UInt · `decimals`: UInt · `show_spread`: Bool (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `level_clicked`: mixed (String parts) · `level_hovered`: mixed (String parts) · `level_unhovered`: mixed (String parts)

**Commands.** `set_series`

**Rust API.** `create_order_book(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("order_book", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `book_ladder`, `market_depth_ladder`

### QuoteBoard (`quote_board`)

A table of instruments with last price, change, volume and sort modes, reporting quote click/hover/selection. Use it as a watchlist; use `OrderBook` for one instrument's ladder and `Table` for non-market data.

**Properties.** 4 published beyond the four shared: `sort`: Enum (`none` | `symbol` | `last_descending` | `change_magnitude`) · `selected_index`: UInt · `row_height`: UInt (read-only) · `scroll_offset`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `quote_clicked`: String · `quote_hovered`: String · `selection_changed`: String or null

**Commands.** `set_series`

**Rust API.** `create_quote_board(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("quote_board", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `quotes`, `watchlist`, `quote_table`, `market_watch`

### IndicatorChart (`indicator_chart`)

A technical-indicator pane for MACD, RSI, stochastic, money flow index, ATR or on-balance volume, with a period and reference levels. Use it beneath a price chart; it consumes the same series rather than owning market data, so the host feeds both panes.

**Properties.** 4 published beyond the four shared: `series`: String · `mode`: Enum (`macd` | `rsi` | `stochastic` | `money_flow_index` | `atr` | `on_balance_volume`) · `period`: UInt · `show_reference_levels`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `bar_clicked`: UInt · `bar_hovered`: UInt · `view_changed`: UInt ×2

**Commands.** `set_series`

**Rust API.** `create_indicator_chart(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("indicator_chart", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `indicator`, `technical_indicator`, `oscillator`, `macd_chart`

## 14. Date, time and advanced selection

The advanced family covers the non-trivial selectors: calendar and date/time editors, the rotary dial, tab and ribbon strips, the pie menu and the keyboard-shortcut editor.

Choose `Calendar` when a month grid is the primary surface, `DatePicker`/`TimePicker`/`DateTimePicker` for one field with a popup, `Dial` for a bounded value manipulated by angle, `TabBar` for a decoupled tab strip, `RibbonBar` for Office-style command tabs, `PieMenu` for a radial secondary action menu, and `KeySequenceEdit` to record a shortcut. Do not use a date editor where a plain `LineEdit` with validation is enough, and do not use `PieMenu` for a long list — radial menus work for a handful of items.

| Family | Source directory | Controls |
|---|---|---|
| `advanced` | `src/widget/advanced_widgets/` | 10 |

### Calendar (`calendar`)

A month grid for browsing and selecting dates, with single, multi or range selection, week numbers, configurable first day and date limits. Use it as the primary date surface; use `DatePicker` for a field with a popup and `DateRangePicker` for a start/end pair with presets.

**Properties.** 12 published beyond the four shared: `selected_date`: String · `minimum_date`: String · `maximum_date`: String · `first_day_of_week`: Enum (`mon` | `tue` | `wed` | `thu` | `fri` | `sat` | `sun`) · `grid_visible`: Bool · `navigation_bar_visible`: Bool · `horizontal_header_visible`: Bool · `vertical_header_visible`: Bool · `date_format`: String · `show_week_numbers`: Bool · `selection_mode`: Enum (`single` | `multi` | `range`) · `selected_dates`: StringList — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `activated`: String · `current_page_changed`: mixed (String parts) · `selection_changed`: String

**Commands.** `set_selected_date`, `set_date_range`, `set_first_day_of_week`, `show_today`

**Rust API.** `create_calendar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("calendar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `date_calendar`

### DatePicker (`date_edit`)

A one-line date field that opens a calendar popup, with a display format, date limits and popup/date-change signals. Use it for one date in a form; use `Calendar` when the grid is always visible and `DateTimePicker` when time is part of the value.

**Properties.** 7 published beyond the four shared: `date`: String · `minimum_date`: String · `maximum_date`: String · `display_format`: String · `calendar_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `calendar_first_day`: Enum (`mon` | `tue` | `wed` | `thu` | `fri` | `sat` | `sun`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `calendar_closed` · `calendar_opened` · `date_changed`: String · `popup_visibility_changed`: Bool

**Commands.** `set_date`, `set_minimum_date`, `set_maximum_date`, `set_display_format`

**Rust API.** `create_date_edit(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("date_edit", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `date_picker`

### TimePicker (`time_edit`)

A one-line time field with a clock popup, 12/24-hour format, time limits and a `time_changed` signal. Use it for a time of day; use `TimePicker` for the picker-flavoured spelling and `DateTimePicker` when a date is also required.

**Properties.** 7 published beyond the four shared: `time`: String · `minimum_time`: String · `maximum_time`: String · `display_format`: String · `clock_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `time_format`: Enum (`12h` | `24h`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `popup_visibility_changed`: Bool · `time_changed`: String

**Commands.** `set_time`, `set_minimum_time`, `set_maximum_time`, `set_display_format`, `set_clock_popup`

**Rust API.** `create_time_edit(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("time_edit", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `time_input`

### TimePicker (`time_picker`)

The `time_picker` factory name for a time field with a clock popup; it shares the `TimePicker` kind and the `TIME_EDIT_PROPERTIES` schema with `time_edit`. Use whichever spelling the host already uses, and do not mount both for one value.

**Properties.** 7 published beyond the four shared: `time`: String · `minimum_time`: String · `maximum_time`: String · `display_format`: String · `clock_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `time_format`: Enum (`12h` | `24h`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `popup_visibility_changed`: Bool · `time_changed`: String

**Commands.** `set_time`, `set_minimum_time`, `set_maximum_time`, `set_display_format`, `set_clock_popup`

**Rust API.** `create_time_picker(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("time_picker", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `time_field`

### DateTimePicker (`date_time_edit`)

A combined date-and-time field with a calendar popup, a display format, date limits and both date and datetime change signals. Use it when one value carries both parts; use `DatePicker` plus `TimePicker` when the two are edited separately.

**Properties.** 7 published beyond the four shared: `datetime`: String · `display_format`: String · `calendar_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `minimum`: String · `maximum`: String · `calendar_first_day`: Enum (`mon` | `tue` | `wed` | `thu` | `fri` | `sat` | `sun`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `calendar_closed` · `calendar_opened` · `date_changed`: String · `datetime_changed`: String · `editing_finished` · `popup_visibility_changed`: Bool · `time_changed`: String

**Commands.** `set_datetime`, `set_display_format`, `set_calendar_popup`

**Rust API.** `create_date_time_edit(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("date_time_edit", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `datetimepicker`, `date_time_picker`

### Dial (`dial`)

A rotary knob that maps its angle to a bounded integer, with notches, a notch target and wrapping. Use it for an instrument-like control where angle is natural (volume, pan, a dial gauge); use `Slider` for a linear range and `SpinBox` for exact entry.

**Properties.** 8 published beyond the four shared: `minimum`: Int · `maximum`: Int · `value`: Int · `single_step`: Int · `page_step`: Int · `notches_visible`: Bool · `notch_target`: Float · `wrapping`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `slider_pressed` · `slider_released` · `value_changed`: Int

**Commands.** `set_range`, `set_value`, `set_wrapping`

**Rust API.** `create_dial(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("dial", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `knob`

### TabBar (`tab_bar`)

A standalone tab strip decoupled from its pages: tab count, current index, closable, movable, min/max width and tab events. Use it when the strip and the page container are separate; use `TabWidget` when the control should own both.

**Properties.** 7 published beyond the four shared: `tab_count`: UInt (read-only) · `current_index`: UInt · `closable`: Bool · `movable`: Bool · `tab_min_width`: UInt · `tab_max_width`: UInt · `direction`: Enum (`ltr` | `rtl`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `current_changed`: UInt · `tab_bar_clicked`: UInt or null · `tab_close_requested`: UInt · `tab_moved`: UInt ×2

**Commands.** `add_tab`, `remove_tab`, `set_current_index`, `set_closable`, `set_movable`

**Rust API.** `create_tab_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("tab_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

### RibbonBar (`ribbon_bar`)

An Office-style ribbon: tabs, groups of commands, large and small items, a quick-access strip and collapse/minimise. Use it for a command-dense desktop app; use `ToolBar` for a single row of actions and `MenuBar` for classic menus.

**Properties.** 6 published beyond the four shared: `tab_count`: UInt (read-only) · `current_tab`: UInt · `expanded`: Bool · `minimized`: Bool · `quick_access_items`: StringList · `quick_access_overflow_menu`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `application_clicked` · `current_tab_changed`: UInt · `group_overflow_clicked`: UInt ×2 · `item_triggered`: UInt ×3 · `quick_access_clicked`: UInt · `quick_access_overflow_clicked`

**Commands.** `add_tab`, `add_group`, `add_item`, `add_large_item`, `clear`

**Rust API.** `create_ribbon_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("ribbon_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `ribbon`

### PieMenu (`pie_menu`)

A radial menu whose items sit on a ring around the pointer, with a radius, inner radius and a current index. Use it for a handful of secondary actions invoked at a point; use `Menu`/`ContextMenu` for a longer or keyboard-navigable list.

**Properties.** 4 published beyond the four shared: `item_count`: UInt (read-only) · `radius`: Float · `inner_radius`: Float · `current_index`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `about_to_hide` · `about_to_show` · `triggered`: UInt · `triggered_text`: String

**Commands.** `add_item`, `remove_item`, `set_radius`, `set_current_index`

**Rust API.** `create_pie_menu(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("pie_menu", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `radial_menu`

### LineEdit (`key_sequence_editor`)

A field that records a keyboard shortcut as a key sequence, reporting conflicts and the new sequence. The schema publishes no property row for it, so drive it through the widget API and its signals rather than through `WidgetFactory` property names.

**Properties.** The schema publishes no property of its own; every control carries the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `conflict_detected`: String · `editing_finished` · `key_sequence_changed`: String · `sequence_changed`: list of String

**Commands.** No commands are published.

**Rust API.** `create_key_sequence_editor(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("key_sequence_editor", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `key_sequence_edit`

## 15. Utility and mobile controls

The miscellaneous family collects small controls that do not need a family of their own: avatars, QR codes, drop zones, date-range selection, segmented buttons, barcode scanning, curve editing and range presets.

Choose `Avatar` for a user image with initials fallback, `QRCode` to render a deterministic code, `DropZone` as a named drop target filtered by MIME type, `DateRangePicker` for a start/end pair, `SegmentedButton` for multi-select segments, `BarcodeScanner` for a camera-driven scan, `BezierCurveEditor` for an easing curve, and `RangePresetBar` beside a range picker. Do not use `DropZone` as a general drag source (implement `DragSource`) and do not use `QRCode` as a barcode decoder — it only renders.

| Family | Source directory | Controls |
|---|---|---|
| `misc` | `src/widget/misc_widgets/` | 9 |

### DropZone (`drop_zone`)

A named drop target filtered by MIME type, with a hover state, a multi-file flag and enter/exit/drop/reject signals. Use it where a host accepts files or a drag payload; use a `DropTarget` implementation directly when the target is an existing control rather than its own region.

**Properties.** 8 published beyond the four shared: `accepted_type`: String · `state`: Enum (`idle` | `hovering` | `accepted` | `rejected` | `dropped`) · `hovered`: Bool (read-only) · `accepted_types`: StringList · `multi_file`: Bool · `hover_style`: String · `show_count_badge`: Bool · `dropped_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `drop_rejected` · `dropped`: list of String · `payload_dropped`: String · `zone_entered`: list of String · `zone_exited`: list of String

**Commands.** No commands are published.

**Rust API.** `create_drop_zone(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("drop_zone", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `drop_target`

### MobileDatePicker (`mobile_date_picker`)

A date picker laid out for touch rather than a desktop calendar grid, with a year span and a date-changed signal. Use it on phone layouts; use `Calendar` or `DatePicker` on desktop, and `CupertinoDatePicker` when the iOS wheel is required.

**Properties.** 2 published beyond the four shared: `selected_date`: String · `year_span`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `date_changed`: String

**Commands.** `set_selected_date`

**Rust API.** `create_mobile_date_picker(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("mobile_date_picker", geometry, text)` and `WidgetFactory::shared()` for the registry.

### QRCode (`qr_code`)

A widget that renders a deterministic QR pattern from a data string, with size, quiet zone, error-correction level, version bounds and colours. Use it to display a code the user scans; it renders only and does not decode, so use `BarcodeScanner` for reading.

**Properties.** 8 published beyond the four shared: `data`: String · `size`: UInt · `quiet_zone`: UInt · `ec_level`: Enum (`L` | `M` | `Q` | `H`) · `min_version`: UInt · `max_version`: UInt · `foreground_color`: Color · `background_color`: Color — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** This control publishes no capability event.

**Commands.** `set_data`, `set_size`

**Rust API.** `create_qr_code(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("qr_code", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `qr`

### Avatar (`avatar`)

A circular or square user image placeholder with initials fallback, a size, an optional badge, an image source and a ring. Use it for a person or account identity; pair it with a `Badge` only when the count belongs elsewhere — the avatar already carries its own `badge` text.

**Properties.** 7 published beyond the four shared: `initials`: String · `square`: Bool · `size`: UInt · `image_source`: String · `badge`: String · `ring_color`: Color · `ring_width`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `avatar_clicked`

**Commands.** `set_initials`, `set_image_source`

**Rust API.** `create_avatar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("avatar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `avatar_widget`, `user_avatar`

### SegmentedButton (`segmented_button`)

A horizontal group of selectable segments carrying ids and icons, with single or multi selection and a group id. Use it for Material 3 style icon segments or multi-choice; use `SegmentedControl` for single-select labelled segments with a sliding indicator and `CupertinoSegmentedControl` for the iOS pill look.

**Properties.** 5 published beyond the four shared: `selected_index`: UInt · `selected_indices`: String · `allows_multiple`: Bool · `segment_count`: UInt (read-only) · `group_id`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `current_changed`: UInt

**Commands.** `set_selected_index`

**Rust API.** `create_segmented_button(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("segmented_button", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `segmented_btn`

### DateRangePicker (`date_range_picker`)

A calendar-based start/end range picker with presets, a double-month view and min/max nights. Use it for booking or reporting ranges; pair it with `RangePresetBar` when the presets must live outside the picker, and use `Calendar` for a single date.

**Properties.** 6 published beyond the four shared: `start_date`: String · `end_date`: String · `presets`: StringList · `double_month`: Bool · `min_nights`: UInt · `max_nights`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `preset_selected`: UInt · `range_changed`: String

**Commands.** `set_start_date`, `set_end_date`

**Rust API.** `create_date_range_picker(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("date_range_picker", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `range_picker`

### BarcodeScanner (`barcode_scanner`)

A scanner viewfinder with a scanning flag and a `barcode_detected` payload. The host owns the camera and decoding; this control owns the scan state and reports the decoded text. Use `QRCode` to render a code, not to read one.

**Properties.** 1 published beyond the four shared: `is_scanning`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `barcode_detected`: String

**Commands.** `set_is_scanning`

**Rust API.** `create_barcode_scanner(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("barcode_scanner", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `scanner`

### BezierCurveEditor (`bezier_curve_editor`)

An interactive cubic Bézier editor for custom easing curves: control points, segment count, snapping and a curve-changed payload. Use it in a motion or animation settings panel; store the emitted curve text and feed it to the animation engine rather than recomputing it.

**Properties.** 3 published beyond the four shared: `snap_to_grid`: Bool · `segments`: UInt (read-only) · `curve_points`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `curve_changed`: mixed (String parts)

**Commands.** `set_snap_to_grid`

**Rust API.** `create_bezier_curve_editor(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("bezier_curve_editor", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `curve_editor`

### RangePresetBar (`range_preset_bar`)

A row of quick date-range chips beside a range picker, with orientation, compact mode and a selected label. The picker owns the dates; the bar only reports the chosen preset, so wire `preset_selected` into `DateRangePicker::apply_preset` or your own mapping.

**Properties.** 4 published beyond the four shared: `presets`: StringList · `orientation`: Enum (`horizontal` | `vertical`) · `compact`: Bool · `selected_label`: String (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `preset_selected`: String ×2

**Commands.** No commands are published.

**Rust API.** `create_range_preset_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("range_preset_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `preset_bar`

## 16. Playback and animation

The media family renders time-varying content: animated and Lottie/Rive animations, hero transitions, video playback, audio levels and a camera preview.

Choose `AnimatedImage` for a frame sequence, `LottieWidget` or `RiveWidget` for vector animation assets, `HeroAnimation` for a shared-element transition between two layouts, `VideoPlayer` for video with transport state, `AudioVisualizer` for a level or spectrum readout, and `CameraPreview` for a viewfinder. Do not use `AnimatedImage` for an interactive animation graph (Rive), and do not use `VideoPlayer` as an audio-only player: its surface and frame signals are wasted.

| Family | Source directory | Controls |
|---|---|---|
| `media` | `src/widget/media_widgets/` | 7 |

### AnimatedImage (`animated_image`)

Plays an animated image (GIF, APNG or a WebP frame sequence) with play/pause, direction, a frame cache limit and frame/animation-finished signals. Use it for a frame sequence asset; use `LottieWidget` or `RiveWidget` for vector animation and `VideoPlayer` for timed media.

**Properties.** 3 published beyond the four shared: `playing`: Bool · `direction`: Enum (`forward` | `reverse`) · `frame_cache_limit`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `animation_finished` · `frame_changed`: UInt

**Commands.** `set_playing`

**Rust API.** `create_animated_image(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("animated_image", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `anim_image`

### HeroAnimation (`hero_animation`)

A shared-element transition that interpolates position, size and opacity between two layouts, with a progress value and a completion signal. Use it when the same object appears on two screens; the host drives `animation_progress` and swaps the real content at the end.

**Properties.** 1 published beyond the four shared: `animation_progress`: Float — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `animation_completed`

**Commands.** `set_animation_progress`

**Rust API.** `create_hero_animation(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("hero_animation", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `hero`

### LottieWidget (`lottie_widget`)

A Lottie JSON animation player with play/pause, loop count, direction and speed. Use it for vector motion assets exported from a design tool; use `RiveWidget` when the animation needs interactive state machines and inputs.

**Properties.** 5 published beyond the four shared: `playing`: Bool · `loop_count`: Int · `loops_completed`: Int (read-only) · `direction`: Enum (`forward` | `reverse`) · `speed`: Float — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `animation_finished`

**Commands.** `set_playing`

**Rust API.** `create_lottie_widget(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("lottie_widget", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `lottie`

### RiveWidget (`rive_widget`)

A Rive runtime surface with play state, loops, a state string and fired-input/state-changed signals. Use it for interactive animation with state machines; use `LottieWidget` for a linear exported animation and `AnimatedImage` for a frame sequence.

**Properties.** 4 published beyond the four shared: `is_playing`: Bool · `loop_count`: Int · `loops_completed`: Int (read-only) · `state`: String (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `animation_finished` · `input_fired`: String · `state_changed`: String

**Commands.** `set_is_playing`

**Rust API.** `create_rive_widget(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("rive_widget", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `rive`

### VideoPlayer (`video_player`)

A video surface with play state, volume, mute, playback rate, controls, looping and a frame/metadata/error signal set. Use it when the host provides the media pipeline; pair it with the transport companions and stop/seek only through the control's own methods so its clock stays authoritative.

**Properties.** 7 published beyond the four shared: `is_playing`: Bool · `volume`: Float · `muted`: Bool · `playback_rate`: Float · `controls_visible`: Bool · `looping`: Bool · `pipeline`: String (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `buffering_progress_changed`: Float · `duration_changed`: Float · `error_occurred`: String · `frame_available`: UInt · `frame_presented`: Float · `fullscreen_changed`: Bool · `media_status_changed`: String · `metadata_changed` · `mute_changed`: Bool · `playback_ended` · `playback_paused` · `playback_rate_changed`: Float · `playback_started` · `time_updated`: Float · `volume_changed`: Float

**Commands.** `set_is_playing`, `set_volume`

**Rust API.** `create_video_player(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("video_player", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `video`

### AudioVisualizer (`audio_visualizer`)

A real-time audio level or spectrum view with bar count, a peak threshold, a mode and periodic level reports. Use it for feedback while audio plays; keep the analysis in the host and feed levels in, and do not use it as an audio editor.

**Properties.** 4 published beyond the four shared: `bar_count`: UInt · `peak_threshold`: Float · `mode`: Enum (`waveform` | `spectrum`) · `level_report_interval`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `level_changed`: Float · `mode_changed`: String · `peak_exceeded`: Float

**Commands.** `set_bar_count`

**Rust API.** `create_audio_visualizer(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("audio_visualizer", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `audio_viz`

### CameraPreview (`camera_preview`)

A camera viewfinder with an active flag, pixel format, target frame rate and camera id, reporting frames, format negotiation, permission denial and errors. Use it for capture or scanning; the host owns the device session, and the control surfaces only what the session reports.

**Properties.** 4 published beyond the four shared: `is_active`: Bool · `pixel_format`: String · `target_fps`: UInt · `camera_id`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `error_occurred`: String · `format_negotiated`: UInt ×2 · `frame_available`: UInt · `frame_presented`: Float · `permission_denied`

**Commands.** `set_is_active`

**Rust API.** `create_camera_preview(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("camera_preview", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `camera`

## 17. Embedded web content

The web family exposes one control, `WebEngineView`, plus the named wrappers (`WebView`, `WebEnginePage`, `WebEngineSettings`, `WebEngineContextMenuRequest`, `WebEngineCookieStore`, `WebEngineDownloadItem`, `WebEngineFindTextResult`, `WebEngineNotification`, `WebEngineScriptDialog`, `WebEngineWebChannel`) that address parts of the same view.

Choose `WebEngineView` when the host must render HTML it does not own — a help page, a third-party sign-in, an embedded dashboard. Do not use it for content the crate can render itself: a `Label` with rich text, a `Canvas`, or a native view avoids the engine's process, memory and security cost. Treat navigation, downloads and script dialogs as events the host must answer.

| Family | Source directory | Controls |
|---|---|---|
| `web` | `src/widget/web_widgets/` | 1 |

### WebEngineView (`web_engine_view`)

The embedded web view: URL, load state, title, back/forward, user agent, transparent background and cookie persistence, with navigation, load, console, download, certificate and crash signals. Use it only for HTML the host does not own; treat each signal as something the host must decide about.

**Properties.** 8 published beyond the four shared: `url`: String · `loading`: Bool (read-only) · `title`: String · `can_go_back`: Bool · `can_go_forward`: Bool · `user_agent`: String · `background_transparent`: Bool · `cookie_persistence_enabled`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `certificate_error`: String · `channel_message`: String ×2 · `console_message`: mixed (String parts) · `download_requested`: String · `error_occurred`: String · `js_evaluated`: mixed (String parts) · `link_hovered`: String · `load_progress_changed`: UInt · `loading_finished`: String · `loading_started`: String · `navigation_state_changed`: Bool ×2 · `page_created`: UInt · `page_destroyed`: UInt · `render_process_crashed`: String · `title_changed`: String · `url_changed`: String

**Commands.** `set_url`, `load_url`, `reload`, `go_back`, `go_forward`, `stop`

**Rust API.** `create_web_view(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("web_engine_view", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `webview`, `web_view`

## 18. Cupertino and Material renditions

This family provides platform-flavoured renditions: iOS-style switch, alert, slider, navigation bar, segmented control and date picker, plus the Material snackbar and navigation rail that complete the adaptive set.

Choose a Cupertino control when the screen must look native on Apple platforms and the host has already decided the platform from a runtime query. Choose `CupertinoSwitch` over `Switch` when the iOS thumb-and-track look is required; it has its own kind, capability row and constructor, so it is not an alias. Do not branch on the target OS in shared code — ask the platform layer at runtime — and do not mix Cupertino and Material chrome in one screen.

| Family | Source directory | Controls |
|---|---|---|
| `cupertino` | `src/widget/cupertino/` | 8 |

### CupertinoSwitch (`cupertino_switch`)

An iOS-style on/off switch with its own kind, capability row and constructor; it is not an alias of `Switch`. Use it when the iOS thumb-and-track rendition is required, and use `Switch` with `switch_geometry` when only the geometry differs.

**Properties.** 4 published beyond the four shared: `checked`: Bool · `thumb_color`: Color · `on_track_color`: Color · `switch_geometry`: Enum (`material` | `cupertino` | `macos`) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `toggled`: Bool

**Commands.** `set_checked`, `toggle`

**Rust API.** `create_cupertino_switch(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("cupertino_switch", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `ios_switch`

### CupertinoAlertDialog (`cupertino_alert_dialog`)

An iOS-style modal alert with a title, a message and confirm/cancel outcomes. Use it for a two-choice iOS alert; use `AlertDialog`/`ConfirmDialog` for the desktop alert family and `MessageBox` for the classic fixed-button form.

**Properties.** 2 published beyond the four shared: `title`: String · `message`: String — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `canceled` · `confirmed`

**Commands.** `set_title`, `set_message`

**Rust API.** `create_cupertino_alert_dialog(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("cupertino_alert_dialog", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `ios_alert`

### CupertinoSlider (`cupertino_slider`)

An iOS-style slider with a rounded track and circular knob, a value and min/max floats. Use it for a continuous value on an iOS-styled screen; use `Slider` for integer steps and `RangeSlider` for two bounds.

**Properties.** 3 published beyond the four shared: `value`: Float · `min`: Float · `max`: Float — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `value_changed`: Float

**Commands.** `set_value`, `set_min`, `set_max`

**Rust API.** `create_cupertino_slider(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("cupertino_slider", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `ios_slider`

### CupertinoNavigationBar (`cupertino_navigation_bar`)

An iOS-style navigation bar with a title, an optional large title, a prompt, leading/trailing items and a collapse offset. Use it at the top of an iOS screen; pair it with the `CupertinoNavigationStack` companion when the title must follow a page stack.

**Properties.** 4 published beyond the four shared: `title`: String · `large_title`: Bool · `prompt`: String · `collapse_offset`: Float — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `back_pressed` · `leading_item_pressed`: UInt · `trailing_item_pressed`: UInt

**Commands.** `set_title`, `set_large_title`

**Rust API.** `create_cupertino_navigation_bar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("cupertino_navigation_bar", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `cupertinonavbar`, `ios_nav_bar`

### CupertinoSegmentedControl (`cupertino_segmented_control`)

An iOS-style pill segmented control with a selected index and a value-changed signal. Use it for two to five mutually exclusive choices on an iOS screen; use `SegmentedControl` for the desktop rendition and `SegmentedButton` for multi-select.

**Properties.** 2 published beyond the four shared: `selected_index`: UInt · `segment_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `value_changed`: UInt

**Commands.** `set_selected_index`

**Rust API.** `create_cupertino_segmented_control(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("cupertino_segmented_control", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `cupertinosegmented`, `ios_segmented`

### CupertinoDatePicker (`cupertino_date_picker`)

An iOS UIPickerView-style scrolling wheel date picker, with date, time, datetime or countdown mode, 24-hour format and a minute interval. Use it inside an iOS-styled sheet; use `MobileDatePicker` on Material touch layouts and `DatePicker` on desktop.

**Properties.** 4 published beyond the four shared: `selected_date`: String · `mode`: Enum (`date` | `time` | `datetime` | `countdown`) · `use_24_hour`: Bool · `minute_interval`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `date_changed`: mixed (String parts) · `time_changed`: UInt ×2

**Commands.** `set_selected_date`

**Rust API.** `create_cupertino_date_picker(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("cupertino_date_picker", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `ios_date_picker`

### MaterialSnackbar (`material_snackbar`)

A Material snackbar with a message, an action label, a duration and a slide progress. Use it for a brief confirmation with one action; use `Banner` for a persistent notice and `ToastStack`/`ToastHost` when several messages must queue.

**Properties.** 4 published beyond the four shared: `message`: String · `action_text`: String · `duration_ms`: UInt · `slide_progress`: Float (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `action_pressed` · `dismissed`

**Commands.** `show`, `dismiss`, `set_message`, `set_action_text`

**Rust API.** `create_material_snackbar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("material_snackbar", geometry, text)` and `WidgetFactory::shared()` for the registry.

### MaterialNavigationRail (`material_navigation_rail`)

A Material side navigation rail for tablet-width layouts, with a selected index and a current-changed signal. Use it when the window is too narrow for a drawer and too wide for a bottom bar; use `NavigationDrawer` for a long destination list and `BottomNavigationBar` on phones.

**Properties.** 1 published beyond the four shared: `selected_index`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `current_changed`: UInt

**Commands.** `set_selected_index`

**Rust API.** `create_material_navigation_rail(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("material_navigation_rail", geometry, text)` and `WidgetFactory::shared()` for the registry.

**Aliases.** `nav_rail`, `navigation_rail`

## 19. macOS renditions

The macOS family holds explicit, separately named controls for native-shaped macOS chrome: a segmented control, a stepper with repeat, a unified toolbar with overflow, a source list and a window chrome bar. Each reports an existing `WidgetKind` but is reached by its own factory name so a host opts in deliberately.

Choose these when a macOS screen must match the platform's metrics and affordances. Do not use them as the default on other platforms, and do not assume they are interchangeable with their generic counterparts: their property tables differ (`mac_stepper` adds repeat delays, `mac_toolbar` adds `selectable`, `mac_window_chrome` adds `document_edited`).

| Family | Source directory | Controls |
|---|---|---|
| `macos` | `src/widget/macos/` | 5 |

### SegmentedControl (`mac_segmented_control`)

The macOS rendition of a segmented control: a selected index, a segment count and a value-changed signal, with platform metrics. Use it in macOS-native chrome; use `SegmentedControl` for the generic control and `CupertinoSegmentedControl` for iOS.

**Properties.** 2 published beyond the four shared: `selected_index`: UInt · `segment_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `value_changed`: UInt

**Commands.** No commands are published.

**Rust API.** `create_mac_segmented_control(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("mac_segmented_control", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Stepper (`mac_stepper`)

The macOS rendition of a stepper, adding `wraps`, `repeat_delay_ms` and `repeat_interval_ms` to the generic value/range/step model. Use it for macOS-native numeric chrome; use `Stepper` for the portable control and `SpinBox` when typing is required.

**Properties.** 7 published beyond the four shared: `value`: Int · `minimum`: Int · `maximum`: Int · `step`: Int · `wraps`: Bool · `repeat_delay_ms`: UInt · `repeat_interval_ms`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `value_changed`: Int

**Commands.** No commands are published.

**Rust API.** `create_mac_stepper(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("mac_stepper", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ToolBar (`mac_toolbar`)

The macOS unified toolbar rendition: items, icon size, orientation, a selectable/selected item and an overflow count. Use it at the top of a macOS window; use `ToolBar` for the portable strip and `RibbonBar` when commands need labelled groups.

**Properties.** 6 published beyond the four shared: `item_count`: UInt (read-only) · `icon_size`: Float · `orientation`: String (read-only) · `selectable`: Bool · `selected_index`: UInt · `overflow_count`: UInt (read-only) — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `item_activated`: UInt · `overflow_activated`: UInt · `overflow_changed`: UInt

**Commands.** No commands are published.

**Rust API.** `create_mac_toolbar(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("mac_toolbar", geometry, text)` and `WidgetFactory::shared()` for the registry.

### ListView (`mac_source_list`)

The macOS source-list rendition of a list: sections, rows, a selected row, row height and badge/selection signals. Use it in a macOS sidebar; use `ListView` for the portable list and pair it with `DrawerSectionHeader` when using the drawer instead.

**Properties.** 4 published beyond the four shared: `section_count`: UInt (read-only) · `row_count`: UInt (read-only) · `row_height`: UInt (read-only) · `selected_row`: UInt — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `badge_changed`: mixed (String parts) · `selection_changed`: String

**Commands.** No commands are published.

**Rust API.** `create_mac_source_list(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("mac_source_list", geometry, text)` and `WidgetFactory::shared()` for the registry.

### Window (`mac_window_chrome`)

The macOS window chrome: title, chrome height, traffic-light buttons, drag/fullscreen/document-edited flags and window lifecycle requests. Use it when a macOS window must match the platform's title bar; for a portable window use `Window`.

**Properties.** 7 published beyond the four shared: `title`: String · `chrome_height`: UInt (read-only) · `buttons_visible`: Bool · `draggable`: Bool · `fullscreen`: Bool · `document_edited`: Bool · `focused`: Bool — plus the four shared properties `enabled`, `visible`, `tooltip` and `geometry`.

**Events.** `close_requested` · `fullscreen_requested` · `minimize_requested` · `window_focus_changed`: Bool · `zoom_requested`

**Commands.** No commands are published.

**Rust API.** `create_mac_window_chrome(geometry, text)` — the registered constructor. Through the factory: `WidgetFactory::create("mac_window_chrome", geometry, text)` and `WidgetFactory::shared()` for the registry.

## Compound widgets: who owns what

Several controls are designed to be used with a partner rather than alone. The table below states, for each pair, which side owns the state, which side paints, and the calls that join them. The same information appears in the relevant control entries; it is collected here so a host wiring a screen can see all of it in one place.

| Primary | Companion | Who owns the state | Who paints / how they are wired |
|---|---|---|---|
| `ColorWell` | `ColorPicker`, `ColorDialog` | The well owns the current colour; the picker owns the editing state. | `picker_requested` opens the picker or dialog; `color_selected` calls `ColorWell::set_color` when the choice is accepted. |
| `ColorDialog` | `Swatch` | The dialog owns the current colour; the swatch owns the palette and its selected index. | `swatch_selected` writes the dialog's colour through `set_hex`; `Swatch::set_selected_index` reflects a colour chosen elsewhere. |
| `ColorPicker` | `ColorHistory` | The picker owns the current colour; the history owns the recent-colour list. | On commit, add the colour to the history; `color_selected` from the history sets the picker's colour. |
| `DataGrid` | `FilterRow` | The grid owns the model and the applied filter; the row owns the condition editors. | `filter_changed` carries the condition text; the grid re-queries and keeps the row's `live` flag in step. |
| `GridTable` | `GridViewHeader` | The grid owns rows, columns and scroll; the header owns the sort/filter affordances and section widths. | Forward `section_clicked` to `toggle_sort_column`, `section_resized` to `set_column_width` and `filter_clicked` to the grid's filter state. |
| `GridTable` | `GridFindBar`, `ColumnChooser` | The grid owns data and selection; the find bar owns its query; the chooser owns column visibility. | The find bar scans the grid's visible columns; the chooser's toggles call the grid's column-visibility setters. |
| `LineChart`, `BarChart`, `PieChart` | `Legend` | The chart owns the series and the visibility flags; the legend owns the entry order and highlight. | `series_toggled` calls the chart's series-visibility setter; the legend rebuilds its entries from the chart's series names. |
| any chart | `ChartTooltip` | The chart owns data and geometry; the tooltip owns only its text lines and visibility. | The chart calls `show_at` with the snapped point on hover and `hide` on leave, filling the tooltip's lines from the hovered datum. |
| any control | `TooltipManager` | The control owns its `tooltip` text; the manager owns the delay, duration and queue. | Feed hover enter/leave to the manager; it shows a `Tooltip` after `delay_ms` and hides it after `duration_ms`. |
| `ImageView` | `ZoomPanController` | The image view owns the pixels and the paint; the controller owns zoom, pan and ROI. | `zoom_changed` calls `ImageView::set_zoom_level`, `pan_changed` calls `set_pan_offset`, and `roi_changed` drives the crop or the detail view. |
| `ImageGallery` | `ThumbnailCache` | The gallery owns the current index and the strip; the cache owns the down-sampled bitmaps. | The gallery asks the cache for a thumbnail by index and paints it; the cache owns eviction and never paints. |
| `MapView` | `TileLoader`, `ZoomControl`, `ScaleBar`, `Compass` | The map owns all four companions (accessors `tile_loader()`, `zoom_control()`, `scale_bar()`, `compass()`); the host owns the tile fetch. | `TileLoader::load_tile` yields a `TileRequest`; the host fetches it and calls `MapView::deliver_tile` or `reject_tile`; `zoom_requested` and `reset_north_requested` are applied by the map. |
| `GanttWidget` | `TimelineAxis`, `DependencyEditor` | The Gantt owns both companions (`timeline_axis()`, `dependency_editor()`) and the task list. | `set_dependencies` / `sync_dependencies_from_tasks` keep the editor and the bars consistent; `pixel_to_date` and `date_to_pixel` come from the axis. |
| `DiffViewer` | `DiffFileList` | The viewer owns both texts, the diff and the selection; the file list owns only the chosen path. | `file_selected` tells the host which path to read; the host then calls `DiffViewer::set_texts`. |
| `MarkdownEditor` | `MarkdownOutlinePanel`, `FindBar` | The editor owns the document, its headings and the cursor; the panel owns only the active outline row; the find bar owns the query. | Build the panel from `outline()`; `entry_selected` calls `goto_outline`; `search_changed` calls `find_next` / `find_previous`. |
| `KanbanBoard` | `KanbanCardView`, `KanbanColumnHeader` | The board owns columns, cards, drag and drop; a card view owns one card's content; a header owns its rename and collapse state. | Build rows from `columns()`/`cards`; `column_renamed` calls the board's column mutator; `card_edited` writes back through the board so drag state is not lost. |
| `DateRangePicker` | `RangePresetBar` | The picker owns the start/end dates; the bar owns the preset labels and the selected chip. | `preset_selected` maps to `DateRangePicker::apply_preset` (or your own date maths); `range_changed` updates the bar's selected label. |
| `FAB` | `SpeedDial` | The FAB owns the collapsed primary action; the speed dial owns the expanded secondary actions. | The FAB's `clicked` toggles the speed dial; each action emits its own intent and the host dismisses the dial after one fires. |
| `Button`, `ToggleButton`, `ToolButton` | `MenuPanel` | The button owns pressed/checked state and its `menu_open` flag; the panel owns the item list and the highlighted row. | Set `menu_open`, anchor the panel below the button, and forward `item_clicked` to the command dispatcher; the panel closes itself on `cancelled`. |
| `SwipeToDismiss` | `SwipeActions` | The container owns the swipe offset and the dismissed state; the actions own the background buttons and their colours. | The container reports the revealed side and offset; the actions paint that band and emit their own intent when one is tapped. |
| `NavigationDrawer` | `DrawerSectionHeader` | The drawer owns open state, selection and width; a section header is a non-selectable label row. | Insert headers between item rows; they do not report selection, so keyboard navigation skips them. |
| `Canvas` | `LayerPanel` | The canvas owns the layers and their order; the panel is a read-only mirror addressed by layer id. | The panel reads layer ids and names; a click asks the canvas to select that id, and the canvas remains the only writer of layer order. |
| `ScrollArea` | `StickyHeader` | The scroll area owns the viewport and the scroll offset; the sticky header owns the content it paints in the band. | Attach it with `ScrollArea::set_sticky_header`; the area offsets its child but keeps the header at the band. |
| `VideoPlayer` | `MediaTransportBar`, `MediaScrubber`, `SubtitleBar` | The player owns the clock, the source and playback state; the companions own their own controls and paint their own band. | Buttons call `play`, `pause`, `seek_to`, `set_volume`; `time_updated` and `duration_changed` drive the scrubber and subtitle timing. |
| `WebEngineView` | `WebFindBar` | The view owns the document and the navigation state; the find bar owns the query and match count. | `search_changed` drives the view's find operation; the view reports the match count back to the bar. |
| `CupertinoNavigationBar` | `CupertinoNavigationStack` | The bar owns title/prompt presentation; the stack owns the page list and the back history. | `stack_changed` updates the bar's title and back visibility; `back_pressed` calls the stack's `pop`. |
| `RichEdit` | `FormatToolbar` | The editor owns the document and the selection; the toolbar owns the format action set and its enabled state. | `FormatAction` values are applied to the current selection; the toolbar's state follows the selection's formatting, not the other way round. |
| `ComboBox`, `AutoCompleteEdit` | `SuggestionPopup` | The field owns the text and the committed value; the popup owns the candidate list and the highlighted index. | The field fills the popup's candidates from its model; `suggestion_selected` writes the chosen value back into the field. |

## WidgetKind coverage

The enum in `src/widget/kind.rs` declares 221 variants. Every one is listed below with the family that documents it. Eleven variants have no capability row of their own: they are alias or child-only kinds, and their entry explains how they resolve.

| Variant | Documented in |
|---|---|
| `Window` | `base` |
| `Dialog` | `dialog` |
| `MessageBox` | `dialog` |
| `FileDialog` | `dialog` |
| `ColorDialog` | `dialog` |
| `FontDialog` | `dialog` |
| `InputDialog` | `dialog` |
| `ProgressDialog` | `dialog` |
| `PopupWindow` | `dialog` |
| `Button` | `base` |
| `CheckBox` | `base` |
| `RadioButton` | `base` |
| `Label` | `base` |
| `LineEdit` | `input` |
| `TextEdit` | `input` |
| `RichEdit` | `input` |
| `ComboBox` | `input` |
| `SpinBox` | `input` |
| `ListBox` | `input` |
| `ListView` | `view` |
| `TreeView` | `view` |
| `ProgressBar` | `display` |
| `Slider` | `display` |
| `ScrollBar` | `display` |
| `ScrollArea` | `container` |
| `Panel` | `base` |
| `Frame` | `base` |
| `DockPanel` | `container` |
| `GroupBox` | `base` |
| `TabWidget` | `container` |
| `Splitter` | `container` |
| `MdiArea` | `container` |
| `MenuBar` | `menu_toolbar` |
| `Menu` | `menu_toolbar` |
| `MenuItem` | `menu_toolbar` |
| `ContextMenu` | `menu_toolbar` |
| `ToolBar` | `menu_toolbar` |
| `StatusBar` | `menu_toolbar` |
| `Canvas` | `special` |
| `Table` | `view` |
| `Grid` | `special` |
| `Chart` | `special` |
| `RadarChart` | `special` |
| `Heatmap` | `special` |
| `KanbanBoard` | `special` |
| `ToggleButton` | `base` |
| `CheckListBox` | `input` |
| `DoubleSpinBox` | `input` |
| `Dial` | `advanced` |
| `Wizard` | `dialog` |
| `DatePicker` | `advanced` |
| `TimePicker` | `advanced` |
| `DateTimePicker` | `advanced` |
| `DirectoryDialog` | `dialog` |
| `DataView` | `view` |
| `EmojiPicker` | `display` |
| `Mention` | `input` |
| `QueryBuilder` | `view` |
| `PropertyGrid` | `view` |
| `Toolbox` | `container` |
| `StackedWidget` | `container` |
| `CollapsiblePane` | `container` |
| `DockWidget` | `container` |
| `ActivityIndicator` | `display` |
| `Calendar` | `advanced` |
| `ColumnView` | `view` |
| `UndoView` | `view` |
| `CommandLink` | `input` |
| `LCDNumber` | `display` |
| `FontComboBox` | `input` |
| `WebEngineView` | `special` |
| `Action` | `menu_toolbar` |
| `ToolButton` | `menu_toolbar` |
| `Cascader` | `input` |
| `FreeformShape` | `special` |
| `TabBar` | `advanced` |
| `PieMenu` | `advanced` |
| `RibbonBar` | `advanced` |
| `Line` | `display` |
| `Meter` | `display` |
| `MiniChart` | `display` |
| `ImageView` | `display` |
| `MiniCanvas` | `display` |
| `Arc` | `display` |
| `Spinner` | `display` |
| `Roller` | `display` |
| `Dropdown` | `input` |
| `TextArea` | `input` |
| `Keyboard` | `input` |
| `Switch` | `display` |
| `SearchBox` | `input` |
| `Chip` | `special` |
| `Badge` | `display` |
| `SkeletonLoader` | `display` |
| `FAB` | `overlay` |
| `BottomSheet` | `dialog` |
| `BottomNavigationBar` | `nav` |
| `NavigationDrawer` | `nav` |
| `AppBar` | `nav` |
| `MobileDatePicker` | `misc` |
| `Divider` | `display` |
| `Stepper` | `container` |
| `Rating` | `display` |
| `Avatar` | `misc` |
| `EmptyState` | `display` |
| `Carousel` | `container` |
| `ColorHistory` | `display` |
| `ColorWell` | `display` |
| `TagInput` | `input` |
| `ImePreedit` | `input` |
| `InplaceEditor` | `input` |
| `QRCode` | `misc` |
| `MasonryLayout` | `container` |
| `CupertinoSwitch` | `cupertino` |
| `MaterialSnackbar` | `cupertino` |
| `AdaptiveScaffold` | `nav` |
| `WizardDialog` | `dialog` |
| `SafeArea` | `container` |
| `CupertinoAlertDialog` | `cupertino` |
| `CupertinoSlider` | `cupertino` |
| `MaterialNavigationRail` | `cupertino` |
| `Tooltip` | `dialog` |
| `SegmentedButton` | `misc` |
| `NavigationStack` | `nav` |
| `ProgressCircle` | `display` |
| `Icon` | `display` |
| `DropdownMenu` | `menu_toolbar` |
| `MaskedEdit` | `input` |
| `MenuButton` | `menu_toolbar` |
| `Popover` | `dialog` |
| `AutoCompleteEdit` | `input` |
| `MultiSelectComboBox` | `input` |
| `RangeSlider` | `input` |
| `FloatingLabel` | `display` |
| `FontPreview` | `display` |
| `CupertinoNavigationBar` | `cupertino` |
| `CupertinoSegmentedControl` | `cupertino` |
| `SwipeToDismiss` | `overlay` |
| `TabView` | `nav` |
| `SearchBar` | `input` |
| `ShortcutEditor` | `input` |
| `RefreshControl` | `overlay` |
| `ModalBottomSheet` | `dialog` |
| `LineChart` | `chart` |
| `Sparkline` | `chart` |
| `BarChart` | `chart` |
| `CandlestickChart` | `finance` |
| `VolumeChart` | `finance` |
| `DepthChart` | `finance` |
| `OrderBook` | `finance` |
| `QuoteBoard` | `finance` |
| `IndicatorChart` | `finance` |
| `FindReplaceDialog` | `dialog` |
| `PropertiesPanel` | `view` |
| `PieChart` | `chart` |
| `CupertinoDatePicker` | `cupertino` |
| `EditableComboBox` | `input` |
| `DateRangePicker` | `misc` |
| `AnimatedImage` | `media` |
| `HeroAnimation` | `media` |
| `BezierCurveEditor` | `misc` |
| `LottieWidget` | `media` |
| `RiveWidget` | `media` |
| `VideoPlayer` | `media` |
| `ImageGallery` | `view` |
| `AudioVisualizer` | `media` |
| `CameraPreview` | `media` |
| `BarcodeScanner` | `misc` |
| `GridTable` | `view` |
| `NumberPicker` | `input` |
| `OtpInput` | `input` |
| `Banner` | `overlay` |
| `Pagination` | `nav` |
| `ColorPicker` | `special` |
| `Toast` | `special` |
| `SplashScreen` | `overlay` |
| `TreeTable` | `view` |
| `Breadcrumb` | `special` |
| `SignaturePad` | `special` |
| `DropZone` | `misc` |
| `SegmentedControl` | `special` |
| `Spacer` | `container` |
| `Center` | `container` |
| `Align` | `container` |
| `SizedBox` | `container` |
| `Padding` | `container` |
| `Expanded` | `container` |
| `Flexible` | `container` |
| `IntrinsicWidth` | `container` |
| `AspectRatio` | `container` |
| `Flex` | `container` |
| `FlowLayout` | `container` |
| `WrapLayout` | `container` |
| `Tag` | `display` |
| `Gauge` | `chart` |
| `Clock` | `display` |
| `Accordion` | `container` |
| `AlertDialog` | `dialog` |
| `ConfirmDialog` | `dialog` |
| `AboutDialog` | `dialog` |
| `Swatch` | `special` |
| `FilterRow` | `special` |
| `Legend` | `special` |
| `ChartTooltip` | `special` |
| `TooltipManager` | `special` |
| `ZoomPanController` | `special` |
| `ToastHost` | `special` |
| `TileLoader` | `special` |
| `ZoomControl` | `special` |
| `ScaleBar` | `special` |
| `Compass` | `special` |
| `TimelineAxis` | `special` |
| `DependencyEditor` | `special` |
| `DiffFileList` | `special` |
| `MarkdownOutlinePanel` | `code_editor` |
| `FindBar` | `code_editor` |
| `ShapeCanvas` | `special` |
| `KanbanCardView` | `special` |
| `KanbanColumnHeader` | `special` |
| `RangePresetBar` | `misc` |
| `GridViewHeader` | `view` |

## Deliberate omissions and boundaries

* **Profile gating.** Most variants are declared under `#[cfg(widgets_unstripped)]`. A stripped (`mini` or `embedded`) build keeps only the always-available subset, so a host targeting such a build must check that a name is constructible rather than assume every control in this chapter exists there.

* **`key_sequence_editor` publishes no property row.** Its schema is empty beyond the four shared properties. The entry says so rather than inventing names; drive the control through its constructor and signals.

* **Alias and child-only kinds.** `Panel`, `DockPanel`, `MenuItem`, `ContextMenu`, `CheckListBox`, `DoubleSpinBox`, `Wizard`, `DirectoryDialog`, `ActivityIndicator`, `ColumnView` and `UndoView` publish no capability row of their own. They are documented where they belong, with the name they resolve to.

* **Companions are controls, not parts of a primary control.** A companion has its own constructor, signals and (except where noted) its own capability row; the pairing table says who owns which state. Painting is always done by the control that owns the geometry it paints into.

* **Properties whose names are self-describing are not paraphrased.** A property named `row_count` of type `UInt` is the row count; repeating that for 700 names would bury the ones that need explanation. Names that are not self-describing are glossed in the control entry.

* **This chapter does not compile the snippets.** They are fragments that use the real constructor and factory shapes; `mdbook test` would need each to be a complete program. `tools/check_cookbook.sh` verifies the identifiers and builds all three books with zero warnings.
