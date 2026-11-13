use super::*;

/// Maximum nested depth for recursive instantiation (prevents stack overflow).
///
/// The value lives in [`crate::json::MAX_DEPTH`] so the loader and the project reader cannot
/// drift (JSON-010).
pub(crate) const MAX_DEPTH: u32 = crate::json::MAX_DEPTH;

/// The largest carousel page index a document may name (W6-08): `index` is untrusted input and
/// the loader grows the carousel up to it, so `4294967295` used to drive billions of allocations.
const MAX_CAROUSEL_PAGES: usize = 1024;

/// The declaration a **published-name** wire was reached from, for its refusal diagnostic.
///
/// # Why the message needs to know
///
/// Two spellings reach one published signal: `events: { "double_clicked": "h" }` and the
/// compatibility key `on_double_click`, whose intent only the published name can carry (W-06). A
/// refusal that always said `events.<name>` was wrong for the second: it named a key the document
/// never wrote and never said what to do instead. The refusal is where an author learns that the
/// key they used has a published name, so it has to name that key.
#[derive(Debug, Clone, Copy)]
pub(crate) enum DeclaredWire {
    /// `events: { "<name>": "<handler>" }` — the author wrote the published name.
    Events,
    /// An `on_*` key whose published counterpart is the channel; carries the key the author wrote.
    Marker(&'static str),
}

/// A loader that parses JSON layout strings and instantiates widget trees.
pub struct JsonLoader;

/// Whether `obj` declares anything [`route_container_content`] would route.
///
/// Used by both the caller (so a leaf node never builds its ancestor chain) and the
/// router itself (so a leaf answers without resolving the canonical widget name).
pub(crate) fn has_routing_key(
    obj: &serde_json::Map<String, Value>,
    child_ids: &[ObjectId],
) -> bool {
    !child_ids.is_empty()
        || obj.contains_key("pages")
        || obj.contains_key("sections")
        || obj.contains_key("panes")
        || obj.contains_key("menus")
        || obj.contains_key("actions")
        || obj.contains_key("items")
}

/// Routes a container node's declared children into that container's own content model.
///
/// # Why this exists (issue-20261104 DM-06/07/08/11/12)
///
/// The generic child path mounts a declared node under its parent and applies layout
/// placement. A container that keeps its content in a **side table** — a page list, a pane
/// list, a slot — was then left with that table empty: a `StackedWidget` painted every page
/// on top of the others, a `ScrollArea` had no viewport child, a `DockArea` had nothing to
/// arrange, and a `MenuBar` had no menus. The mount itself is correct; only the routing was
/// missing.
///
/// Every entry point used here is the control's **own existing method**, and every one of
/// them takes the `ObjectId` the mount already assigned. Nothing about ownership moves: the
/// runtime registry stays the single owner of the widget, and the tree painter still reaches
/// every child. That is the property `CapabilityValue` cannot express — it has no
/// control-reference variant (`capability/types.rs`) — which is why routing is a loader
/// concern rather than a property.
///
/// # Keys
///
/// A single-slot container takes its **first** declared child as content. Multi-slot
/// containers read a dedicated array (`pages`, `sections`, `panes`, `docks`, `items`,
/// `menus`, `actions`) whose entries are nodes in the same `{ "<type>": { … } }` shape as
/// `children`. Every one of these keys is registered in [`is_loader_owned_key`], so the
/// name-driven property pass does not report them as ignored.
#[allow(clippy::too_many_arguments)]
pub(crate) fn route_container_content(
    widget_id: ObjectId,
    widget_type: &str,
    obj: &serde_json::Map<String, Value>,
    child_ids: &[ObjectId],
    registry: &mut crate::index::WidgetRegistry,
    binding: &mut BoundJsonLayout,
    depth: u32,
    resolver: Option<&dyn crate::widget::icon_source::IconResourceResolver>,
    ancestors: &[ElementRef<'_>],
) -> Result<(), String> {
    // Guarded first: a leaf node has neither children nor a content key, so the caller
    // skips building its ancestor chain and this function answers without allocating the
    // canonical name (DM5-01).
    if !has_routing_key(obj, child_ids) {
        return Ok(());
    }
    let canonical = crate::widget::capability::WidgetFactory::shared().canonical_name(widget_type);
    let first = child_ids.iter().copied().find(|id| *id != 0);

    match canonical.as_str() {
        // ── Single-slot containers ─────────────────────────────────────────
        //
        // `ScrollArea::set_widget`, `CollapsiblePane::set_content`,
        // `SafeArea::set_content`, `DockWidget::set_widget` and the dialog family's
        // `set_content_widget` all link the id, place it in the slot rectangle and keep
        // its visibility in step. They are idempotent, so the mount's existing child
        // entry is not duplicated (see `BaseWidget::add_child_linked`).
        "scroll_area" => {
            with_container::<ScrollArea>(widget_id, |area| {
                area.set_widget(first);
            });
        }
        "collapsible_pane" => {
            with_container::<CollapsiblePane>(widget_id, |pane| {
                if let Some(id) = first {
                    pane.set_content(id);
                }
            });
        }
        "safe_area" => {
            with_container::<SafeArea>(widget_id, |area| {
                area.set_content(first);
            });
        }
        "dock_widget" | "dock_area" => {
            with_container::<DockWidget>(widget_id, |dock| {
                dock.set_widget(first);
            });
        }
        "frame" => {
            with_container::<Frame>(widget_id, |frame| {
                frame.set_widget(first);
            });
        }
        "dialog" => {
            with_container::<Dialog>(widget_id, |dialog| {
                dialog.set_content_widget(first);
            });
        }
        "bottom_sheet" => {
            with_container::<BottomSheet>(widget_id, |sheet| {
                sheet.set_content_widget(first);
            });
        }
        "popup_window" => {
            with_container::<PopupWindow>(widget_id, |popup| {
                popup.set_content_widget(first);
            });
        }
        "modal_bottom_sheet" => {
            with_container::<ModalBottomSheet>(widget_id, |sheet| {
                sheet.set_content_id(first);
            });
        }
        // `TabView` draws one page at a time and draws each page itself, so its content has
        // to be a tab entry rather than a plain child.
        "tab_view" => {
            if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
                for (position, child_value) in children.iter().enumerate() {
                    let Some(id) = child_ids.get(position).copied().filter(|id| *id != 0) else {
                        continue;
                    };
                    let Some((child_type, child_val)) =
                        child_value.as_object().and_then(|wrapper| wrapper.iter().next())
                    else {
                        continue;
                    };
                    let title = child_val
                        .get("title")
                        .and_then(|v| v.as_str())
                        .or_else(|| child_val.get("text").and_then(|v| v.as_str()))
                        .or_else(|| child_val.get("id").and_then(|v| v.as_str()))
                        .unwrap_or(child_type)
                        .to_string();
                    with_container::<TabView>(widget_id, |view| {
                        view.add_tab_id(title, Some(id), None::<alloc::string::String>);
                    });
                }
            }
        }

        // ── Multi-slot containers ─────────────────────────────────────────
        //
        // `StackedWidget::add_widget` is the exact shape the plan asks for: the children
        // are the pages, in document order, and the control hides every page but the
        // selected one. `current_index` is applied by the property pass afterwards, which
        // is why nothing is written here.
        "stacked_widget" => {
            with_container::<StackedWidget>(widget_id, |stack| {
                for id in child_ids.iter().copied().filter(|id| *id != 0) {
                    stack.add_widget(id);
                }
            });
        }

        // A splitter's panes are declared under `panes`, each carrying its own `stretch`.
        "splitter" => {
            let panes = node_array(obj, "panes");
            if !panes.is_empty() {
                for (child_type, child_val) in panes {
                    let id = JsonLoader::instantiate_node(
                        child_type,
                        child_val,
                        Some(widget_id),
                        registry,
                        binding,
                        depth + 1,
                        resolver,
                        ancestors,
                    )?;
                    if id == 0 {
                        continue;
                    }
                    let stretch =
                        child_val.as_object().and_then(|m| json_u32_at(m, "stretch")).unwrap_or(1);
                    with_container::<Splitter>(widget_id, |splitter| {
                        splitter.add_pane(id, stretch);
                    });
                }
            }
        }

        // An accordion's sections are declared under `sections`; each entry is one node
        // whose `text`/`title` names the section and whose `id` is the section identity.
        "accordion" => {
            let sections = node_array(obj, "sections");
            for (child_type, child_val) in sections {
                let id = JsonLoader::instantiate_node(
                    child_type,
                    child_val,
                    Some(widget_id),
                    registry,
                    binding,
                    depth + 1,
                    resolver,
                    ancestors,
                )?;
                if id == 0 {
                    continue;
                }
                let section_id = child_val
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| alloc::format!("section_{id}"));
                let title = child_val
                    .get("title")
                    .and_then(|v| v.as_str())
                    .or_else(|| child_val.get("text").and_then(|v| v.as_str()))
                    .unwrap_or(child_type)
                    .to_string();
                let body = child_val.get("body").and_then(|v| v.as_str()).unwrap_or("").to_string();
                with_container::<Accordion>(widget_id, |accordion| {
                    accordion.add_section(&section_id, &title, &body);
                    accordion.set_section_widget(&section_id, Some(id));
                });
            }
        }

        // A wizard's steps are declared under `pages`; the page widget is bound to the
        // step the control just created.
        "wizard_dialog" => {
            let pages = node_array(obj, "pages");
            for (child_type, child_val) in pages {
                let id = JsonLoader::instantiate_node(
                    child_type,
                    child_val,
                    Some(widget_id),
                    registry,
                    binding,
                    depth + 1,
                    resolver,
                    ancestors,
                )?;
                if id == 0 {
                    continue;
                }
                let title = child_val
                    .get("title")
                    .and_then(|v| v.as_str())
                    .or_else(|| child_val.get("text").and_then(|v| v.as_str()))
                    .unwrap_or(child_type)
                    .to_string();
                let optional = child_val.get("optional").and_then(|v| v.as_bool()).unwrap_or(false);
                with_container::<WizardDialog>(widget_id, |wizard| {
                    let index = wizard.add_page(title, optional);
                    wizard.set_page(index, Some(id));
                });
            }
        }

        // `dock_area` has no multi-dock arm here: its capability row resolves to
        // `create_dock_area`, which builds a **`DockWidget`** (a single panel), because
        // `DockArea` is a layout owner rather than a `Widget` and has no mounted instance a
        // document could address. The single-slot arm above therefore serves both names, and
        // a multi-dock host still has to be described in Rust.

        // A navigation stack's `children` are its history: the first child is the root, each
        // later one is pushed on top of it. Only the topmost page is visible.
        "navigation_stack" => {
            with_container::<NavigationStack>(widget_id, |stack| {
                for id in child_ids.iter().copied().filter(|id| *id != 0) {
                    stack.push_id(id);
                }
            });
        }

        // The three linear containers address their children through an item list, so a plain
        // tree child would be arranged by nothing. See `FlexItemHost`.
        "flex" => {
            if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
                route_flex_children::<Flex>(widget_id, children, child_ids);
            }
        }
        "flow_layout" => {
            if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
                route_flex_children::<FlowLayout>(widget_id, children, child_ids);
            }
        }
        "wrap_layout" => {
            if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
                route_flex_children::<WrapLayout>(widget_id, children, child_ids);
            }
        }
        // A masonry card is its own row of data: label, height and face colour. The label falls
        // back through the same chain the tab and accordion arms use, the height is the child's
        // declared `height`, and the face colour is its declared `background` — a declared card is
        // a mounted control, so an absent `background` leaves the face transparent rather than
        // inventing a colour the document did not ask for.
        "masonry_layout" => {
            if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
                for (position, child_value) in children.iter().enumerate() {
                    let Some(id) = child_ids.get(position).copied().filter(|id| *id != 0) else {
                        continue;
                    };
                    let Some((child_type, child_val)) =
                        child_value.as_object().and_then(|wrapper| wrapper.iter().next())
                    else {
                        continue;
                    };
                    let Some(node) = child_val.as_object() else { continue };
                    let label = node
                        .get("text")
                        .and_then(|v| v.as_str())
                        .or_else(|| node.get("title").and_then(|v| v.as_str()))
                        .or_else(|| node.get("id").and_then(|v| v.as_str()))
                        .unwrap_or(child_type)
                        .to_string();
                    let height =
                        json_u32_at(node, "height").unwrap_or(DEFAULT_DECLARED_CARD_HEIGHT);
                    let face = node
                        .get("background")
                        .and_then(|v| v.as_str())
                        .and_then(Color::parse_hex)
                        .unwrap_or(Color::TRANSPARENT);
                    with_container::<MasonryLayout>(widget_id, |masonry| {
                        masonry.add_item_with_widget(&label, height, face, Some(id));
                    });
                }
            }
        }

        // A carousel declares its pages under `pages`, each entry naming a page index and the
        // mounted child that fills it (DM-11's last container). The page's title and colour come
        // from the child's own declaration, the same derivation the masonry cards use: the slot
        // is layout data, and inventing a schema for it would be a second source of truth.
        "carousel" => {
            for (position, page) in as_array(obj.get("pages")).iter().enumerate() {
                let Some(entry) = page.as_object() else { continue };
                let declared = entry
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(position);
                let index = declared.min(MAX_CAROUSEL_PAGES - 1);
                if declared != index {
                    log::warn!("[json] carousel page index {declared} clamped to {index}");
                }
                let Some((child_type, child_val)) =
                    entry.iter().find(|(key, _)| key.as_str() != "index")
                else {
                    continue;
                };
                let id = JsonLoader::instantiate_node(
                    child_type,
                    child_val,
                    Some(widget_id),
                    registry,
                    binding,
                    depth + 1,
                    resolver,
                    ancestors,
                )?;
                if id == 0 {
                    continue;
                }
                let node = child_val.as_object();
                // The page's title and colour come from the child's own declaration, the same
                // derivation the masonry cards use: a page's slot is layout data, and inventing a
                // second schema for it would be a second source of truth.
                let title = node
                    .and_then(|node| node.get("text").and_then(|v| v.as_str()))
                    .or_else(|| node.and_then(|n| n.get("title").and_then(|v| v.as_str())))
                    .unwrap_or("")
                    .to_string();
                let color = node
                    .and_then(|node| node.get("background").and_then(|v| v.as_str()))
                    .and_then(Color::parse_hex)
                    .unwrap_or(Color::TRANSPARENT);
                with_container::<Carousel>(widget_id, |carousel| {
                    // Pages are positional: a document may name page 2 before page 1, so the
                    // carousel grows to the named index before the child is bound.
                    while carousel.page_count() <= index {
                        carousel.add_page("", Color::TRANSPARENT);
                    }
                    carousel.set_page_title(index, &title);
                    carousel.set_page_color(index, color);
                    carousel.set_page_content_id(index, Some(id));
                });
            }
        }

        // A tool box declares its items under `items`; the node is the item's page.
        "tool_box" => {
            let items = node_array(obj, "items");
            for (child_type, child_val) in items {
                let id = JsonLoader::instantiate_node(
                    child_type,
                    child_val,
                    Some(widget_id),
                    registry,
                    binding,
                    depth + 1,
                    resolver,
                    ancestors,
                )?;
                if id == 0 {
                    continue;
                }
                let text = child_val
                    .get("text")
                    .and_then(|v| v.as_str())
                    .or_else(|| child_val.get("title").and_then(|v| v.as_str()))
                    .unwrap_or(child_type)
                    .to_string();
                with_container::<ToolBox>(widget_id, |toolbox| {
                    toolbox.add_item(text, Some(id));
                });
            }
        }

        // ── Remaining containers whose content is its `children` ──────────
        //
        // `MdiArea::add_sub_window` takes the geometry the child declared, because a
        // sub-window is a rectangle inside the MDI client area. `StatusBar::add_widget_id`
        // takes the declared `stretch`, matching the splitter's rule.
        "mdi_area" => {
            // The children were mounted by the generic child path, so they are paired with
            // `child_ids` by position rather than instantiated a second time.
            if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
                for (position, child_value) in children.iter().enumerate() {
                    let Some(id) = child_ids.get(position).copied().filter(|id| *id != 0) else {
                        continue;
                    };
                    let geometry = child_value
                        .as_object()
                        .and_then(|m| m.values().next())
                        .and_then(|v| v.as_object())
                        .map(json_geometry)
                        .unwrap_or_else(|| json_geometry(obj));
                    with_container::<MdiArea>(widget_id, |area| {
                        area.add_sub_window(id, geometry);
                    });
                }
            }
        }
        "adaptive_scaffold" => {
            // A declared child fills the slot its `slot` key names (DM-06), and a declared
            // page fills a bottom-navigation destination (DM-08).
            if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
                for (position, child_value) in children.iter().enumerate() {
                    let Some(id) = child_ids.get(position).copied().filter(|id| *id != 0) else {
                        continue;
                    };
                    // `slot` is a key on the child's own node object — the same object that
                    // carries `id`/`text`/`width` — not on the one-key wrapper around it.
                    let Some(slot) = child_value
                        .as_object()
                        .and_then(|wrapper| wrapper.values().next())
                        .and_then(|node| node.get("slot"))
                        .and_then(|v| v.as_str())
                    else {
                        continue;
                    };
                    match lowered(slot).as_ref() {
                        "body" => {
                            with_container::<AdaptiveScaffold>(widget_id, |scaffold| {
                                scaffold.set_body_id(Some(id));
                            });
                        }
                        "fab" => {
                            with_container::<AdaptiveScaffold>(widget_id, |scaffold| {
                                scaffold.set_fab_id(Some(id));
                            });
                        }
                        "drawer" => {
                            with_container::<AdaptiveScaffold>(widget_id, |scaffold| {
                                scaffold.set_drawer_id(Some(id));
                            });
                        }
                        "end_drawer" | "enddrawer" => {
                            with_container::<AdaptiveScaffold>(widget_id, |scaffold| {
                                scaffold.set_end_drawer_id(Some(id));
                            });
                        }
                        "bottom_sheet" | "bottomsheet" => {
                            with_container::<AdaptiveScaffold>(widget_id, |scaffold| {
                                scaffold.set_bottom_sheet_id(Some(id));
                            });
                        }
                        "snackbar" => {
                            with_container::<AdaptiveScaffold>(widget_id, |scaffold| {
                                scaffold.set_snackbar_id(Some(id));
                            });
                        }
                        // The app bar's own bottom strip (DM-11). It used to be the one slot with
                        // no id entry; it now places a declared child like every other slot.
                        "bottom" => {
                            with_container::<AdaptiveScaffold>(widget_id, |scaffold| {
                                scaffold.set_app_bar_bottom_id(Some(id));
                            });
                        }
                        other => {
                            log::warn!(
                                "JSON layout: 'adaptive_scaffold' has no content slot named \
                                 '{other}'; the child stays a plain tree child"
                            );
                        }
                    }
                }
            }
            for (position, page) in as_array(obj.get("pages")).iter().enumerate() {
                let Some(entry) = page.as_object() else { continue };
                let index = entry
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(position);
                let Some((child_type, child_val)) =
                    entry.iter().find(|(name, _)| name.as_str() != "index")
                else {
                    continue;
                };
                let id = JsonLoader::instantiate_node(
                    child_type,
                    child_val,
                    Some(widget_id),
                    registry,
                    binding,
                    depth + 1,
                    resolver,
                    ancestors,
                )?;
                if id == 0 {
                    continue;
                }
                with_container::<AdaptiveScaffold>(widget_id, |scaffold| {
                    scaffold.set_nav_body_id(index, Some(id));
                });
            }
        }
        "status_bar" => {
            with_container::<StatusBar>(widget_id, |bar| {
                for id in child_ids.iter().copied().filter(|id| *id != 0) {
                    bar.add_widget_id(id, 1);
                }
            });
        }

        // ── Menu / toolbar families (DM-12) ───────────────────────────────
        //
        // A menu bar's top-level entries are declared under `menus`, each with a title and
        // an `items` array whose fields are the ones `Action`/`MenuEntry` already carry.
        "menu_bar" => {
            for menu_value in as_array(obj.get("menus")) {
                let Some(menu_obj) = menu_value.as_object() else { continue };
                let title = menu_obj
                    .get("title")
                    .and_then(|v| v.as_str())
                    .or_else(|| menu_obj.get("text").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();
                let entries = as_array(menu_obj.get("items"));
                let mut menu = Menu::new(title.clone(), json_geometry(obj));
                for entry in entries {
                    if entry.get("separator").and_then(|v| v.as_bool()) == Some(true) {
                        menu.add_separator();
                        continue;
                    }
                    let text = entry.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    match entry.get("shortcut").and_then(|v| v.as_str()) {
                        Some(shortcut) => {
                            menu.add_action_with_shortcut(text, shortcut);
                        }
                        None => {
                            menu.add_action(text);
                        }
                    }
                }
                with_container::<MenuBar>(widget_id, |bar| {
                    let index = bar.add_menu(title);
                    bar.attach_menu(index, menu);
                });
            }
        }
        "tool_bar" => {
            let geometry = json_geometry(obj);
            for action_value in as_array(obj.get("actions")) {
                let Some(action_obj) = action_value.as_object() else { continue };
                let separator = action_obj.get("separator").and_then(|v| v.as_bool()) == Some(true);
                let action = if separator {
                    Action::separator(geometry)
                } else {
                    let text = action_obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    let mut action = Action::new(text, geometry);
                    if let Some(shortcut) = action_obj.get("shortcut").and_then(|v| v.as_str()) {
                        action.set_shortcut(shortcut);
                    }
                    if let Some(checkable) = action_obj.get("checkable").and_then(|v| v.as_bool()) {
                        action.set_checkable(checkable);
                    }
                    if let Some(checked) = action_obj.get("checked").and_then(|v| v.as_bool()) {
                        action.set_checked(checked);
                    }
                    action
                };
                with_container::<ToolBar>(widget_id, |bar| {
                    bar.add_action_ref(&action);
                });
            }
        }

        _ => {}
    }
    Ok(())
}

/// The linear containers that hold [`FlexItemSpec`] items.
///
/// `flex`, `flow_layout` and `wrap_layout` each own a *list* of items — the solver addresses them by
/// index, and a real child is attached through [`FlexItemSpec::with_widget`]. They have three
/// separate `add_item` methods and no common trait, so this names the one operation routing needs
/// rather than repeating the item-construction code three times (issue-20261104 DM-11 ⑯).
pub(crate) trait FlexItemHost: Widget + 'static {
    /// Appends `item`, linking the child it carries when it has one.
    fn add_flex_item(&mut self, item: FlexItemSpec);
}

impl FlexItemHost for Flex {
    fn add_flex_item(&mut self, item: FlexItemSpec) {
        self.add_item(item);
    }
}

impl FlexItemHost for FlowLayout {
    fn add_flex_item(&mut self, item: FlexItemSpec) {
        self.add_item(item);
    }
}

impl FlexItemHost for WrapLayout {
    fn add_flex_item(&mut self, item: FlexItemSpec) {
        self.add_item(item);
    }
}

/// Routes a declared child list into a linear container's item list.
///
/// Each child becomes one item whose label is the child's text/title/id and whose extent is the
/// child's own declared `width`/`height` — the same numbers the generic child path already used as
/// its starting geometry, so the item and the control agree about the size before the first solve.
pub(crate) fn route_flex_children<T: FlexItemHost>(
    widget_id: ObjectId,
    children: &[Value],
    child_ids: &[ObjectId],
) {
    for (position, child_value) in children.iter().enumerate() {
        let Some(id) = child_ids.get(position).copied().filter(|id| *id != 0) else { continue };
        let Some((child_type, child_val)) =
            child_value.as_object().and_then(|wrapper| wrapper.iter().next())
        else {
            continue;
        };
        let Some(node) = child_val.as_object() else { continue };
        let label = node
            .get("text")
            .and_then(|v| v.as_str())
            .or_else(|| node.get("title").and_then(|v| v.as_str()))
            .or_else(|| node.get("id").and_then(|v| v.as_str()))
            .unwrap_or(child_type);
        let size = crate::core::Size::new(
            json_u32_at(node, "width").unwrap_or(100),
            json_u32_at(node, "height").unwrap_or(100),
        );
        let item = FlexItemSpec::new(label, size).with_widget(id);
        with_container::<T>(widget_id, |host| host.add_flex_item(item));
    }
}

/// The height a declared masonry card gets when its node does not say.
///
/// A card is a layout row, not a control, so it has no published `height` property to fall back on.
/// This is the loader's own default, named so a reader can find it and so a document that omits the
/// key gets the same card every time.
pub(crate) const DEFAULT_DECLARED_CARD_HEIGHT: u32 = 120;

/// Runs `f` on the mounted control at `widget_id` when it is a `T`.
///
/// Answers whether the control was the expected type. A miss is not an error: the document
/// named a control the routed arm did not expect, and the name-driven property pass has
/// already had its say. Returning the answer keeps the callers honest about the distinction
/// between "routed" and "skipped" without logging the same mismatch twice.
pub(crate) fn with_container<T: Widget + 'static>(
    widget_id: ObjectId,
    f: impl FnOnce(&mut T),
) -> bool {
    let mut applied = false;
    crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        if let Some(typed) = crate::widget::capability::coercion::widget_as_mut::<T>(widget) {
            f(typed);
            applied = true;
        }
    });
    applied
}

/// The `{ "<type>": { … } }` nodes of a declared array, in document order.
///
/// The same shape `children` uses, so a container's dedicated key (`pages`, `sections`,
/// `panes`, `docks`, `items`) reads like the generic one and the loader has a single
/// definition of "a node". A malformed entry is skipped rather than aborting the load: the
/// rest of the document is still constructible.
pub(crate) fn node_array<'a>(
    obj: &'a serde_json::Map<String, Value>,
    key: &str,
) -> alloc::vec::Vec<(&'a str, &'a Value)> {
    let mut nodes = alloc::vec::Vec::new();
    for value in as_array(obj.get(key)) {
        let Some(entry) = value.as_object() else { continue };
        if entry.len() != 1 {
            continue;
        }
        if let Some((name, node)) = entry.iter().next() {
            nodes.push((name.as_str(), node));
        }
    }
    nodes
}

/// The array at `value`, or an empty slice when it is absent or not an array.
pub(crate) fn as_array(value: Option<&Value>) -> &[Value] {
    value.and_then(|value| value.as_array()).map(|v| v.as_slice()).unwrap_or(&[])
}
impl JsonLoader {
    /// Parse a JSON layout string and instantiate the widget tree.
    ///
    /// Returns a [`BoundJsonLayout`] for typed widget access.
    ///
    /// # Incremental reloads (G-406 / RW-43)
    ///
    /// This entry parses and instantiates **once**: loading the same edited document again builds
    /// a second independent tree, so focus, scroll and selection on unchanged controls are lost.
    /// A designer's edit-reload loop should use [`JsonLoader::session`], which forwards the same
    /// parsed document through `crate::view::ViewEngine` and patches only what changed. This
    /// full-mount path remains the one that applies every loader-owned key (geometry, spacing,
    /// colours, construction-time switches).
    ///
    /// # Breakpoint tiers (RW-117)
    ///
    /// A node may declare `"breakpoints": ["compact", "medium", "expanded"]`; the loader honours
    /// the tier in force through `crate::view::Breakpoint::current`, the same fact the incremental
    /// session and `Node::breakpoint` read. A host that loads at a known window size wraps this call:
    ///
    /// ```text
    /// with_breakpoint(Breakpoint::of(size), || JsonLoader::load(json))
    /// ```
    ///
    /// Without a wrapper the tier is `Expanded`, the honest "no viewport reported" answer.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON parsing fails, an unknown widget type is
    /// encountered, or the widget tree exceeds `MAX_DEPTH`.
    pub fn load(json_str: &str) -> Result<BoundJsonLayout, String> {
        Self::load_with_optional_icon_resolver(json_str, None)
    }

    /// Opens an incremental session over this loader's parser (G-406 / RW-43).
    ///
    /// The returned [`JsonViewSession`](crate::json::JsonViewSession) mounts a document through
    /// `ViewEngine` and updates an edited one by diffing it against what is mounted, so unchanged
    /// controls keep their ids (and their focus, scroll and selection). It is a fresh session, not
    /// a global: two editors over two documents do not share a mounted tree.
    #[cfg(declarative_view)]
    pub fn session() -> crate::json::JsonViewSession {
        crate::json::JsonViewSession::new()
    }

    /// Load a JSON layout while resolving resource-backed `icon_source` properties explicitly.
    ///
    /// The resolver owns resource identity and policy; it is never treated as a file path by this
    /// loader. [`Self::load`] remains resolver-free and reports unresolved resources through the
    /// normal property-rejection diagnostic path.
    pub fn load_with_icon_resolver(
        json_str: &str,
        resolver: &dyn crate::widget::icon_source::IconResourceResolver,
    ) -> Result<BoundJsonLayout, String> {
        Self::load_with_optional_icon_resolver(json_str, Some(resolver))
    }

    fn load_with_optional_icon_resolver(
        json_str: &str,
        resolver: Option<&dyn crate::widget::icon_source::IconResourceResolver>,
    ) -> Result<BoundJsonLayout, String> {
        let value: Value = serde_json::from_str(json_str).map_err(|e| {
            format!("layout JSON ({} bytes) could not be parsed: {e}", json_str.len())
        })?;
        let mut registry = crate::index::WidgetRegistry::new();
        let mut binding = BoundJsonLayout::new();

        let root = value.as_object().ok_or_else(|| "JSON root must be an object".to_string())?;

        // The root should have exactly one top-level key (the widget type).
        if root.len() != 1 {
            return Err(format!(
                "JSON root must have exactly one widget type, found {} keys",
                root.len()
            ));
        }

        let (widget_type, widget_value) = match root.iter().next() {
            Some(pair) => pair,
            None => {
                return Err("JSON root object is empty — expected a widget type key".to_string())
            }
        };
        Self::instantiate_node(
            widget_type,
            widget_value,
            None,
            &mut registry,
            &mut binding,
            0,
            resolver,
            &[],
        )?;

        // Run layout diagnostics after all widgets are created.
        LayoutInspector::run_once_logged(&registry);

        Ok(binding)
    }

    /// Wires the handlers a node declares, for both routes.
    ///
    /// # Why one function for two key spellings
    ///
    /// The published route (`events`) and the compatibility route (`on_*`) reach the same
    /// [`EventHandlerContext`] through the same marker table — see `crate::json::event_route`.
    /// Splitting them gave the loader two code paths that had to agree about trigger kinds and
    /// about which handlers are invoked, with nothing checking that they did.
    ///
    /// # What is reported rather than ignored
    ///
    /// A name under `events` that the control does not publish is **dropped with a warning**
    /// rather than silently wired. A designer that misspells a published name would otherwise
    /// get a handler that never runs; the warning is what makes that visible in the host's log.
    /// The reverse case — an `on_*` key the marker table does not list — cannot reach here,
    /// because the table *is* the list of keys this reads.
    fn bind_declared_events(
        widget_id: ObjectId,
        widget_type: &str,
        obj: &serde_json::Map<String, Value>,
    ) {
        // ── route 1: published names ──
        if let Some(events) = obj.get(crate::json::EVENTS_KEY).and_then(|v| v.as_object()) {
            for (name, handler) in events {
                let Some(handler) = handler.as_str() else {
                    log::warn!(
                        "[{widget_type}] `{EVENTS_KEY}.{name}` must be a handler name string, \
                         found {found:?}; the binding is skipped",
                        EVENTS_KEY = crate::json::EVENTS_KEY,
                        found = handler
                    );
                    continue;
                };
                if !Self::control_publishes(widget_type, name) {
                    log::warn!(
                        "[{widget_type}] `{EVENTS_KEY}.{name}` is not a published event \
                         (publishes: {published}); the binding is skipped, so the handler \
                         would never run",
                        EVENTS_KEY = crate::json::EVENTS_KEY,
                        published = Self::published_events(widget_type).join(", ")
                    );
                    continue;
                }
                Self::bind_one(
                    widget_id,
                    crate::json::json_event_binding(widget_type, name),
                    handler.to_owned(),
                );
            }
        }

        // ── route 2: compatibility keys ──
        for (key, marker) in crate::json::MARKER_KEYS {
            let Some(handler) = obj.get(*key).and_then(|v| v.as_str()) else {
                continue;
            };
            Self::bind_one(
                widget_id,
                crate::json::JsonEventBinding::Marker { key, marker: *marker },
                handler.to_owned(),
            );
        }
    }

    /// Subscribes one declared handler to `widget_id` through the callback the event needs.
    ///
    /// # Additive, not idempotent (BLUE-issue E-22)
    ///
    /// Calling this twice with the same `(widget_id, binding, handler_name)` subscribes the handler
    /// **twice**: one control emission runs it twice. That is deliberate \u2014 two legitimate
    /// bindings that happen to share a handler name are two subscribers \u2014 but it means this
    /// call is not a safe "ensure wired" operation. A host that must be able to release a single
    /// dynamic binding it added (a designer replacing one wire without destroying the control)
    /// should use [`Self::bind_one_releasable`], which returns a token; this convenience form
    /// retains it in `LOADED_BINDINGS` for the load-time case where the whole layout is torn down
    /// together.
    ///
    /// # Both routes yield a retained token (W-04)
    ///
    /// A published name and an `on_*` marker each produce a [`crate::json::DynamicBinding`], and both
    /// are retained keyed by the control. The marker route used to bind and report no token, so
    /// unloading or rebuilding a document released the published half of its wiring and left the
    /// marker half connected until the control itself was destroyed. The public shape is unchanged
    /// (`bool` in, `bool` out); the token channel is what got completed.
    pub fn bind_one(
        widget_id: ObjectId,
        binding: crate::json::JsonEventBinding,
        handler_name: String,
    ) -> bool {
        match Self::bind_wiring(widget_id, binding, handler_name) {
            Some(token) => {
                // The binding must outlive this call (it is the load-time wire), so its token is
                // **retained** keyed by the control rather than dropped or forgotten. Dropping it
                // here would release the subscription immediately; forgetting it leaked the token's
                // captured signal reference past the control's own destruction (BLUE-issue E-27). It
                // is released when the control is torn down or its node detached.
                crate::json::event_route::retain_binding(widget_id, token);
                true
            }
            None => false,
        }
    }

    /// Builds the subscription for `binding` and the token that releases it, without retaining it.
    ///
    /// # Why one constructor for both callers
    ///
    /// [`Self::bind_one`] retains the token as the document's wire and
    /// [`Self::bind_one_releasable`] hands the same token to a host that owns its own wire. Having
    /// the marker route return a bare `bool` from a *different* function is what let the token gap
    /// (W-04) survive: the published path had a handle to give back and the marker path did not, so
    /// only one of them was releasable.
    fn bind_wiring(
        widget_id: ObjectId,
        binding: crate::json::JsonEventBinding,
        handler_name: String,
    ) -> Option<crate::json::DynamicBinding> {
        match binding.published_name() {
            Some(name) => Self::bind_published_dynamic(
                widget_id,
                name,
                binding.marker(),
                handler_name,
                DeclaredWire::Events,
            ),
            None => Self::bind_marker_wiring(widget_id, binding.marker(), handler_name),
        }
    }

    /// Wires one `on_*` marker through the channel its intent names.
    ///
    /// # The decision is the marker's, not this function's
    ///
    /// Three questions on [`crate::json::JsonTriggerMarker`], asked in this order, are the whole
    /// routing table — there is no per-key `match` here for a new key to drift away from:
    ///
    /// 1. `base_signal` — the widget's own `clicked` / `closed` / `focus_gained` / `focus_lost`.
    /// 2. `uses_value_callback` — the app layer's value-trigger channel.
    /// 3. `published_counterpart` — a published name for an intent no widget callback can carry
    ///    (`on_double_click`), wired through the control's own dynamic signal for that name.
    ///
    /// A marker with none of the three is refused with its stated reason rather than attached to a
    /// callback that fires on a different event (rule #97).
    fn bind_marker_wiring(
        widget_id: ObjectId,
        marker: crate::json::JsonTriggerMarker,
        handler_name: String,
    ) -> Option<crate::json::DynamicBinding> {
        // 1. A base-widget signal: `clicked`, `closed`, and the two focus signals the runtime's
        //    focus router emits through `BaseWidget::handle_event` (W-05).
        if let Some(signal) = marker.base_signal() {
            return Self::bind_base_signal(widget_id, marker, signal, handler_name);
        }

        // 2. The value-trigger channel: `on_change` / `on_value_changed` / `on_selection_changed`.
        if marker.uses_value_callback() {
            return Self::bind_value_trigger(widget_id, marker, handler_name);
        }

        // 3. An intent no widget callback can carry (W-06). Its published name is the channel, and
        //    the control has to publish it: the loader must not fall back to the click callback,
        //    which would run the handler on the first click of a double click. The refusal names the
        //    key the author wrote, because a document that declared `on_double_click` needs to know
        //    which published name carries it.
        if let Some(name) = marker.published_counterpart() {
            return Self::bind_published_dynamic(
                widget_id,
                name,
                marker,
                handler_name,
                crate::json::marker_key_for(marker)
                    .map_or(DeclaredWire::Events, DeclaredWire::Marker),
            );
        }

        let reason = marker.unroutable_reason().unwrap_or(
            "this `on_*` key has no base signal, no value trigger and no published name to reach",
        );
        log::warn!(
            "on_* key for handler '{handler_name}' on id={widget_id} was NOT bound: {reason}"
        );
        crate::app::record_unwired_binding(widget_id);
        None
    }

    /// Connects a marker to one of the widget's own base signals.
    ///
    /// # Why the JSON layer connects the signal itself (W-04)
    ///
    /// `WidgetHandle::on_click` / `on_close` connect these same signals but keep the
    /// `ConnectionHandle` in a private per-widget table, so a wire made through them cannot be
    /// removed without removing every callback the control has — which is not a release, it is a
    /// demolition. A [`crate::json::DynamicBinding`] needs a real disconnect, so the signal is cloned
    /// out of the live control and subscribed here: the same `base.clicked` / `base.closed` /
    /// `base.focus_gained` / `base.focus_lost` connection the handle would have made, with the
    /// handle kept where the token can reach it.
    ///
    /// A control's teardown drops its signals, so a binding released here and a binding whose
    /// control is destroyed both end with nothing running; the difference is that this one can be
    /// released while the control is still alive.
    fn bind_base_signal(
        widget_id: ObjectId,
        marker: crate::json::JsonTriggerMarker,
        signal: crate::json::event_route::BaseSignal,
        handler_name: String,
    ) -> Option<crate::json::DynamicBinding> {
        use crate::json::event_route::BaseSignal;
        let source = crate::widget::runtime::with_widget(widget_id, |widget| {
            let base = widget.base();
            match signal {
                BaseSignal::Clicked => base.clicked.clone(),
                BaseSignal::Closed => base.closed.clone(),
                BaseSignal::FocusGained => base.focus_gained.clone(),
                BaseSignal::FocusLost => base.focus_lost.clone(),
            }
        });
        let Some(source) = source else {
            Self::report_unwired_marker(widget_id, &handler_name);
            return None;
        };
        let release_source = source.clone();
        let handle = source.connect(move || {
            let ctx = crate::json::context_for(widget_id, marker);
            crate::json::invoke_global_handler(&handler_name, &ctx);
        });
        Some(crate::json::DynamicBinding::new(alloc::boxed::Box::new(move || {
            release_source.disconnect(handle);
        })))
    }

    /// Connects a marker to the app layer's value-trigger channel.
    ///
    /// # Why this channel's token is a liveness flag (W-04)
    ///
    /// `WidgetHandle::on_value_changed` is the only writer of the value-trigger table, and that table
    /// is keyed by widget rather than by subscription — it exposes no per-callback removal. A
    /// document-level release therefore flips this binding's own flag, so a released wire cannot run
    /// its handler; the registration itself is reclaimed when the control is torn down
    /// (`app::remove_callbacks`). The alternative — reporting a token whose `release` silently did
    /// nothing — is the fake release the token exists to rule out, so the boundary is stated here
    /// instead of left for a reader to discover.
    ///
    /// The flag is per binding, not per handler name: two nodes that name the same handler are two
    /// subscriptions, and releasing one must leave the other running.
    fn bind_value_trigger(
        widget_id: ObjectId,
        marker: crate::json::JsonTriggerMarker,
        handler_name: String,
    ) -> Option<crate::json::DynamicBinding> {
        if !crate::widget::runtime::is_mounted(widget_id) {
            // Ask the same question `on_value_changed` asks, so a token is never handed out for a
            // wire that was not installed.
            Self::report_unwired_marker(widget_id, &handler_name);
            return None;
        }
        let live = crate::compat::Arc::new(core::sync::atomic::AtomicBool::new(true));
        let gate = crate::compat::Arc::clone(&live);
        let handle: ButtonHandle = ButtonHandle::from_raw(widget_id);
        handle.on_value_changed(move |_value: &str| {
            if !gate.load(core::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let ctx = crate::json::context_for(widget_id, marker);
            crate::json::invoke_global_handler(&handler_name, &ctx);
        });
        Some(crate::json::DynamicBinding::new(alloc::boxed::Box::new(move || {
            live.store(false, core::sync::atomic::Ordering::Relaxed);
        })))
    }

    /// Reports a marker binding that had no live channel, at the level a host can see.
    ///
    /// A handler that can never run is the failure a subscriber cannot observe from the outside, so
    /// it is reported at `warn!` and counted by `app::unwired_binding_count`. Unlike
    /// [`Self::bind_marker_wiring`]'s refusal this case is about the *control*: it is not there.
    fn report_unwired_marker(widget_id: ObjectId, handler_name: &str) {
        log::warn!(
            "the `on_*` key for handler '{handler_name}' on id={widget_id} was NOT bound: the id \
             addresses no live widget, so there is no signal for its trigger to arrive on"
        );
        crate::app::record_unwired_binding(widget_id);
    }

    /// Like [`Self::bind_one`], but returns a token that releases exactly the binding it added.
    ///
    /// # Both routes return a token (W-04)
    ///
    /// The published route's subscription is on the control's own dynamic signal, whose handle is
    /// the only key that can remove it; the marker route now yields a token too — a real disconnect
    /// for the base signals it connects, and a liveness flag for the value-trigger table, which has
    /// no per-callback removal (see `Self::bind_value_trigger`). A host that added a wire at run
    /// time can therefore take it back without destroying the control, whichever spelling the
    /// document used; returning `None` now means the binding was *refused*, not that it was made
    /// silently.
    pub fn bind_one_releasable(
        widget_id: ObjectId,
        binding: crate::json::JsonEventBinding,
        handler_name: String,
    ) -> Option<crate::json::DynamicBinding> {
        Self::bind_wiring(widget_id, binding, handler_name)
    }

    /// Wires a **published** event name to the control's own dynamic signal.
    ///
    /// # Why this rather than the generic callback tables
    ///
    /// `Widget::event_signal_dyn` is the single join from a published name to the signal the control
    /// actually emits (see [`crate::signal::EventSignalBinder`], which wires `forward_all` through the
    /// same call). Binding a published name through `base.clicked` instead was a second, weaker join
    /// that ignored the name: every payload-free event resolved to the click signal, so `canceled`
    /// fired for a click and never for a cancel. Routing through `event_signal_dyn` also delivers the
    /// event's **payload** to the handler ([`EventHandlerContext::payload`]), which the generic
    /// callbacks discarded.
    ///
    /// # Returns
    ///
    /// `None`, with a warning, when the control does not resolve the name. That case means the name
    /// is published but has no live signal — the exact "subscribed successfully but will never fire"
    /// state rule #97 rules out — so it is reported rather than silently falling back to a callback
    /// that would run on the wrong event.
    ///
    /// Otherwise `Some(token)`, whose `release` disconnects **exactly** this subscription. Two calls
    /// with the same arguments subscribe twice (additive, BLUE-issue E-22), and each token releases
    /// its own subscription, so a host that replaces one dynamic wire can do so without disturbing
    /// the others or destroying the control.
    fn bind_published_dynamic(
        widget_id: ObjectId,
        name: &'static str,
        marker: crate::json::JsonTriggerMarker,
        handler_name: String,
        declared_as: DeclaredWire,
    ) -> Option<crate::json::DynamicBinding> {
        let reference =
            crate::widget::runtime::with_widget(widget_id, |widget| widget.event_signal_dyn(name));
        let Some(reference) = reference.flatten() else {
            match declared_as {
                DeclaredWire::Events => log::warn!(
                    "`events.{name}` for handler '{handler_name}' on id={widget_id} was NOT bound: \
                     the control does not resolve this published name to a signal, so the handler \
                     would never run"
                ),
                DeclaredWire::Marker(key) => log::warn!(
                    "`{key}` for handler '{handler_name}' on id={widget_id} was NOT bound: it is \
                     carried by the published event `{name}`, which this control does not publish \
                     or cannot resolve — see the document's `events` validation output for the \
                     names it does publish. The binding is skipped rather than attached to a \
                     callback that fires on a different event"
                ),
            }
            crate::app::record_unwired_binding(widget_id);
            return None;
        };
        // The slot forwards the trigger **and its payload**. The reference's own name is used (not
        // the caller's spelling) so the name a subscriber reads is the one the control answers to.
        //
        // # Thread affinity (BLUE-issue E-25, identity corrected in E-26)
        //
        // The handler registry is thread-local, so a signal emitted from another thread looked up
        // that thread's (empty) registry and skipped the handler **silently**. The binding records the
        // thread it was made on and reports a mismatch instead of pretending the handler ran, so
        // "fired off the wrong thread" is observable rather than indistinguishable from "no handler
        // was registered".
        //
        // The comparison is on the thread's **`ThreadId`**, not its name: names are not unique, so two
        // threads sharing a name compared equal and one's emission was mis-taken for a same-thread
        // delivery (BLUE-issue E-26). The names are still captured for the diagnostic message only.
        //
        // # Why the comparison no longer costs a `Thread` handle per emit (W-P07)
        //
        // The binding thread's id is captured once, here. The *current* id is read through
        // `events::current_thread_id`, which caches it per thread: it used to call
        // `std::thread::current()` on every emission, and that clones the thread's `Arc` handle — an
        // atomic increment on the hot path whose only use was to read an immutable id. A driver of a
        // value-carrying signal (a slider drag) pays this per emit.
        let bound_thread_id = crate::json::events::current_thread_id();
        let bound_thread_name = crate::json::events::current_thread_name();
        let handle = reference.subscribe(Box::new(
            move |payload: &crate::widget::capability::CapabilityValue| {
                if crate::json::events::current_thread_id() != bound_thread_id {
                    crate::json::events::record_cross_thread_skip(
                        &handler_name,
                        &bound_thread_name,
                        &crate::json::events::current_thread_name(),
                    );
                    return;
                }
                // # Why the payload clone is skipped when nobody listens (W-P07)
                //
                // The context owns its payload, so building one for a name with no registered
                // handler clones the value — a `String`/`List` payload allocates — and then drops it
                // unread. `has_global_handler` reads the same thread-local registry
                // `invoke_global_handler` would, so skipping here is exactly the state in which the
                // registry would have reported "not found": delivery semantics are unchanged.
                if !crate::json::events::has_global_handler(&handler_name) {
                    return;
                }
                let ctx = crate::json::context_for_with_payload(widget_id, marker, payload.clone());
                crate::json::invoke_global_handler(&handler_name, &ctx);
            },
        ));
        // Resolve once more for the release closure: `EventSignalRef` is not `Clone`, and its
        // `disconnect` is per-call, so the token captures a fresh lookup and the handle it removes.
        let mut release_reference =
            crate::widget::runtime::with_widget(widget_id, |widget| widget.event_signal_dyn(name))
                .flatten();
        let release = alloc::boxed::Box::new(move || {
            if let Some(reference) = release_reference.as_mut() {
                reference.disconnect(handle);
            }
        });
        Some(crate::json::DynamicBinding::new(release))
    }

    /// Places a child in the region it declared, or at the cell it declared.
    ///
    /// # Why this exists at all
    ///
    /// `ChildLayoutAttrs::from_value` parsed `col`, `row`, `col_span` and `row_span` into
    /// [`ChildLayoutAttrs`], and both `add_widget_to_layout` call sites then read `attrs.stretch`
    /// and dropped the other four fields. A document that placed a control at an explicit cell
    /// therefore got auto-placement, silently — the parse was the only evidence the key had ever
    /// been seen, and it was thrown away one line later. The keys were also absent from
    /// [`is_loader_owned_key`], so the name-driven property pass additionally reported them as
    /// *"no construction arm consumed it; the value was ignored"* — a warning about a value this
    /// function now consumes.
    ///
    /// # Why a non-grid parent is reported rather than ignored
    ///
    /// `col`/`row` mean nothing to a box, form or flex layout; the trait's `add_widget` carries no
    /// cell. The child is still placed (by the parent's own rule) rather than dropped, but the
    /// mismatch is logged, because a document that wrote a cell on a non-grid parent asked for
    /// something the parent cannot do and would otherwise read as "the grid ignored my column".
    ///
    /// A declared `region` takes precedence over a declared cell: a child cannot be in two
    /// placement systems at once, and the region names a stronger constraint (a fixed track) than
    /// a cell does.
    fn apply_child_placement(
        child_id: ObjectId,
        attrs: &ChildLayoutAttrs,
        child_type: &str,
        parent_id: ObjectId,
        binding: &BoundJsonLayout,
    ) {
        crate::json::layout::apply_child_layout_placement(
            child_id,
            attrs,
            child_type,
            parent_id,
            &|name| binding.id(name),
        );
    }

    /// Whether `widget_type` publishes `event_name`.
    ///
    /// # Why the answer is "no" on a stripped profile
    ///
    /// The capability table is compiled out of `mini`/`embedded`, and `crate::json` does not
    /// exist there either — this function is inside a `full_widgets` module, so the table is
    /// always present when it runs. The fallback is stated rather than assumed because a future
    /// gate change would otherwise turn it into a silent `true`.
    fn control_publishes(widget_type: &str, event_name: &str) -> bool {
        let factory = crate::widget::capability::WidgetFactory::shared();
        factory.capability(widget_type).is_some_and(|capability| {
            crate::widget::capability::find_event_schema(capability, event_name).is_some()
        })
    }

    /// The published event names of `widget_type`, for the warning above.
    fn published_events(widget_type: &str) -> Vec<&'static str> {
        let factory = crate::widget::capability::WidgetFactory::shared();
        factory
            .capability(widget_type)
            .map(|capability| capability.events.iter().map(|schema| schema.name).collect())
            .unwrap_or_default()
    }

    /// Recursively instantiate a single JSON node into a widget.
    #[allow(clippy::too_many_arguments)]
    fn instantiate_node(
        widget_type: &str,
        value: &Value,
        parent_id: Option<ObjectId>,
        registry: &mut crate::index::WidgetRegistry,
        binding: &mut BoundJsonLayout,
        depth: u32,
        resolver: Option<&dyn crate::widget::icon_source::IconResourceResolver>,
        ancestors: &[ElementRef<'_>],
    ) -> Result<ObjectId, String> {
        if depth > MAX_DEPTH {
            return Err(format!(
                "widget tree is nested {depth} levels deep, which exceeds the maximum of \
                 {MAX_DEPTH}; flatten the layout to load it"
            ));
        }

        let obj = value
            .as_object()
            .ok_or_else(|| format!("'{widget_type}' value must be a JSON object"))?;

        // RW-117: a node's `breakpoints` list selects whether it exists in the tier currently in
        // force. Checked before the spacer/layout branches so an excluded pseudo-node creates no
        // layout entry either.
        if !breakpoints_include(obj, widget_type, parent_id.is_none())? {
            return Ok(0);
        }

        // The node's name, lowercased once per node rather than once per test against it.
        // Three comparisons below ask "is this a button/label/window", and each used to spell
        // `widget_type.to_lowercase()` — a fresh `String` per question, per node, for a name
        // that is lowercase in the document almost every time.
        let widget_key = lowered(widget_type);

        // Get the widget ID (optional — auto-generated if missing).
        let id_str = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");

        // Handle the "spacer" pseudo-widget
        if widget_type.eq_ignore_ascii_case("spacer") {
            let stretch = json_u32_at(obj, "stretch").unwrap_or(1);
            if let Some(pid) = parent_id {
                add_spacer_to_layout(stretch, pid);
            }
            return Ok(0);
        }

        // Handle the "layout" pseudo-widget
        if widget_type.eq_ignore_ascii_case("layout") {
            let kind = parse_layout_kind(value)?;
            let layout = create_layout_from_kind(&kind);
            let layout_parent = parent_id
                .ok_or_else(|| format!("'{widget_type}' layout must be a child of a widget"))?;

            // Stored *before* the children are instantiated, because registering a
            // child needs the layout to exist (`add_widget_to_layout` is a no-op
            // otherwise). Storing it afterwards — which this code used to do — left the
            // layout empty, so `apply_layout` computed an arrangement for no children
            // and no widget ever moved.
            store_layout(layout_parent, layout);

            // Process children
            if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
                for child_value in children {
                    if let Some(child_obj) = child_value.as_object() {
                        if child_obj.len() == 1 {
                            let (child_type, child_val) = child_obj.iter().next().unwrap();
                            if child_type.eq_ignore_ascii_case("spacer") {
                                let stretch = child_val
                                    .as_object()
                                    .and_then(|m| json_u32_at(m, "stretch"))
                                    .unwrap_or(1);
                                add_spacer_to_layout(stretch, layout_parent);
                                continue;
                            }

                            let child_id = Self::instantiate_node(
                                child_type,
                                child_val,
                                Some(layout_parent),
                                registry,
                                binding,
                                depth + 1,
                                resolver,
                                ancestors,
                            )?;

                            if child_id != 0 {
                                let attrs = ChildLayoutAttrs::from_value(child_val);
                                // The layout must already be stored for this to reach
                                // it: `add_widget_to_layout` is a no-op when the parent
                                // has none. `store_layout` used to run *after* this
                                // loop, so every child was added to nothing and the
                                // layout was empty when it was applied — which is why
                                // a declarative layout could compute geometries that no
                                // widget ever received.
                                add_widget_to_layout(child_id, attrs.stretch, layout_parent);
                                Self::apply_child_placement(
                                    child_id,
                                    &attrs,
                                    child_type,
                                    layout_parent,
                                    binding,
                                );
                            }
                        }
                    }
                }
            }

            apply_layout(layout_parent, json_geometry(obj));
            return Ok(layout_parent);
        }

        // Create the widget
        let mut widget: Box<dyn Widget> = Self::create_widget(widget_type, obj)?;

        // The id the document wrote under `"id"`. Read here rather than after the
        // mount because the mount is what makes the id real: a widget's own `id()` is
        // only meaningful once it is registered, and the registry hands out the id.
        let declared_id = if id_str.is_empty() { 0 } else { parse_declared_id(id_str) };

        // Apply stylesheet rules first, so explicit JSON keys below win over the
        // stylesheet — the same precedence a browser gives inline styles over an
        // author stylesheet. `class` selects which rules apply; the widget's own
        // kind is matched automatically.
        apply_declared_styles(&mut *widget, obj, ancestors);

        // Apply common properties (geometry, enabled, visible, tooltip, style).
        //
        // The min/max size constraints are applied **inside** this call: `apply_properties`
        // ends with `apply_size_constraints`. Calling it here as well was a second path to
        // the same concept (principle #101). It is idempotent today, which is exactly why the
        // duplicate was invisible — and exactly why a later edit that made it non-idempotent
        // (accumulating a constraint, say) would break silently, with the second call hidden
        // behind the first.
        apply_properties_with_icon_resolver(&mut *widget, obj, resolver);

        // Set parent
        widget.set_parent(parent_id);

        // Register — **mounting** the control, not merely indexing it.
        //
        // # The defect this closes
        //
        // The loader used to record the widget's *own* `id()` in `WidgetRegistry` and
        // `BoundJsonLayout` and drop the `Box<dyn Widget>` on the floor. Nothing reached
        // `crate::widget::runtime`, which is what `with_widget`/`with_widget_mut` read —
        // so every handle a caller resolved from this binding (`widget_by_name::<..>`, the
        // `on_click`/`on_value_changed` wiring, `set_widget_text`) addressed an id the
        // runtime had never heard of. Each of those paths *reports* the miss, but at
        // `debug!`, so at the default log level a loaded document was a tree of controls
        // that returned their defaults and ignored their handlers, with nothing said.
        //
        // The mount goes through the control backend's creation funnel, which is the one
        // place that assigns an id, registers it, applies the active theme, records both
        // directions of the parent/child relation, and gives a window its host surface.
        // Doing it by hand here would be a second implementation of that, and the second
        // one is the one that rots.
        // The element's kind is captured before the mount consumes the box; the ancestor chain is
        // built lazily from this value below (DM5-01).
        let node_kind = widget.kind();
        let widget_id = mount_declared_widget(widget, parent_id, declared_id, widget_type);
        if widget_id == 0 && !id_str.is_empty() {
            // Name the document's own spelling of the id. The backend and mount warnings
            // this path already logs can only name the numeric id and the widget type, so
            // an author with two `"id": "submit_btn"` nodes could not tell which line to
            // fix: the load succeeds and the duplicate node simply has no live control.
            log::warn!(
                "[json] the declared id '{id_str}' could not be claimed; another widget \
                 already holds it and this node was not mounted"
            );
        }

        // The **live control's** kind, not a value derived from the type name.
        //
        // `infer_kind` is a hand-written table covering about a hundred names, while
        // the factory routes far more (canonical names plus aliases). Anything the
        // table did not name — `icon`, `table`, `chip`, `progress_bar`, … — fell
        // through its `_ => WidgetKind::Button` arm, so the registry recorded `Button`
        // for a control that is not one, and `LayoutInspector`/`by_kind` queries then
        // reported the wrong kind. The widget already knows its kind, so reading it
        // here removes the table's ability to disagree with reality.
        //
        // `infer_kind` is still the fallback for the case the live control cannot answer:
        // a thread with no widget runtime, so nothing was mounted and there is no `kind()`
        // to read. `Panel` is the neutral answer there — a control that did not mount has no
        // kind this loader can observe, and reporting `Button` (what the old table's `_` arm
        // did) was a claim about a widget that in fact does not exist.
        let kind = crate::widget::runtime::with_widget(widget_id, |live| live.kind())
            .unwrap_or(crate::index::WidgetKind::Panel);

        // The index exists only for the layout inspector's diagnostic report, which is opt-in
        // (`LayoutInspector::is_enabled` defaults to false). Accumulating an entry per node on
        // every JSON load was work no reader could observe (XC-05).
        if LayoutInspector::is_enabled() {
            let label = if id_str.is_empty() {
                format!("{widget_type}_{widget_id}")
            } else {
                id_str.to_string()
            };
            registry.register(crate::index::WidgetEntry {
                id: widget_id,
                kind,
                parent: parent_id,
                label,
            });
        }

        if !id_str.is_empty() {
            binding.register(id_str, widget_id);
        }

        // Structural index: record which node this is, under which parent, and with which
        // key. `id_str` doubles as the key because the JSON schema already requires it to be
        // unique per document (it is how `widget_by_name` addresses a control), so requiring
        // a second, separate `key` field would let the two disagree. A node without an `id`
        // therefore has no stable identity to diff on, and `node_key` reports `None` for it.
        binding.register_node(widget_id, widget_type, id_str, parent_id);

        // ── Apply text property via platform API ────────────
        // Text for widgets that accept it (checkbox, radiobutton,
        // groupbox title via "title" key, lineedit placeholder, etc.)
        // is set through the global set_widget_text function.
        //
        // i18n: If the text value is a translation key, resolve it
        // BEFORE passing to the loader (e.g. tr!("button.ok")).
        // The loader uses the literal string as-is.
        if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
            // `messagebox` consumed `text` (and `title`) in its `create_widget` arm, so the
            // post-pass below would set it a second time — one avoidable round trip and a
            // second text-changed signal per message box.
            if !text.is_empty() && !matches!(widget_key.as_ref(), "button" | "label" | "messagebox")
            {
                crate::set_widget_text(widget_id, text);
            }
        }
        // Handle "title" property — BLUE4.md spec says groupbox uses "title"
        // (not "text"). For window, title is already set in create_widget.
        // Only apply via platform API for non-window widgets. The same applies to
        // the kinds whose `create_widget` arm already consumed `title` (groupbox/panel,
        // messagebox, filedialog): applying it again here would deliver the same
        // text twice.
        if !matches!(
            widget_key.as_ref(),
            "window" | "groupbox" | "panel" | "messagebox" | "filedialog"
        ) {
            if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                if !title.is_empty() {
                    crate::set_widget_text(widget_id, title);
                }
            }
        }

        // ── Event binding ──────────────────────────────────────────────
        //
        // Two routes, one binder. See `crate::json::event_route` for why both exist and which
        // job each one has:
        //
        //   * `"events": { "<published_name>": "<handler>" }` — the published route. The name is
        //     checked against the control's capability, so a published event is declarable with
        //     no edit here (rule #101).
        //   * `"on_*": "<handler>"` — the compatibility route. Its keys mean a trigger *intent*
        //     (`on_close` → Closed) rather than a published name, and they are read from
        //     `MARKER_KEYS` rather than from a hand-written tuple, so a key cannot be extracted
        //     into the wrong position.
        Self::bind_declared_events(widget_id, widget_type, obj);

        // The ids the mount assigned, in document order. The generic child path below only
        // mounts and places a child; a container that keeps its content in a side table
        // (a page list, a pane list, a slot) needs these ids afterwards. See
        // [`route_container_content`].
        let mut child_ids: alloc::vec::Vec<ObjectId> = alloc::vec::Vec::new();
        // A `tabwidget`'s children are its **pages**, not a stack of siblings. A document
        // that nests panels under a tab widget means "one page each", and the container's
        // whole contract is that exactly one of them is visible. Without this branch the
        // loader merely mounted each child with the tab widget as parent: the tab strip had
        // no tabs and every page painted at its declared rectangle, so the control looked
        // like one panel holding all of its sub-panels at once.
        let tab_widget_children = widget_type.eq_ignore_ascii_case("tabwidget");

        // Builds this node's ancestor chain on demand. A leaf node (no children, no layout, no
        // container content) never calls it, so it no longer allocates a `Vec` for a chain no
        // consumer reads (DM5-01).
        let ancestor_chain = || {
            let mut chain = ancestors.to_vec();
            chain.push(ElementRef {
                kind: node_kind,
                class: obj.get("class").and_then(|v| v.as_str()),
                id: if id_str.is_empty() { None } else { Some(id_str) },
                state: None,
            });
            chain
        };

        // Handle children (for container widgets)
        if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
            let child_ancestors = ancestor_chain();
            for child_value in children {
                if let Some(child_obj) = child_value.as_object() {
                    if child_obj.len() == 1 {
                        let (child_type, child_val) = child_obj.iter().next().unwrap();
                        let child_id = Self::instantiate_node(
                            child_type,
                            child_val,
                            Some(widget_id),
                            registry,
                            binding,
                            depth + 1,
                            resolver,
                            &child_ancestors,
                        )?;
                        // Pushed even when the node was skipped (a `breakpoints` exclusion
                        // returns 0), so `child_ids` stays index-aligned with the document's
                        // `children` array. The slot-routed arms pair the two by position.
                        child_ids.push(child_id);
                        if tab_widget_children && child_id != 0 {
                            let title = child_val
                                .get("title")
                                .and_then(|v| v.as_str())
                                .or_else(|| child_val.get("text").and_then(|v| v.as_str()))
                                .or_else(|| child_val.get("id").and_then(|v| v.as_str()))
                                .unwrap_or(child_type);
                            crate::widget::runtime::with_widget_mut(widget_id, |widget| {
                                if let Some(tabs) =
                                    crate::widget::capability::coercion::widget_as_mut::<TabWidget>(
                                        widget,
                                    )
                                {
                                    tabs.add_tab(title.to_string(), Some(child_id));
                                }
                            });
                        }
                    }
                }
            }
            // `current_index` was applied when the control was constructed, before the pages
            // existed, so `set_current_index` had nothing to select. Apply it now that the tab
            // list is complete; the last write wins, which is what the document asked for.
            if tab_widget_children {
                if let Some(index) = json_usize_at(obj, "current_index") {
                    crate::widget::runtime::with_widget_mut(widget_id, |widget| {
                        if let Some(tabs) =
                            crate::widget::capability::coercion::widget_as_mut::<TabWidget>(widget)
                        {
                            tabs.set_current_index(index);
                        }
                    });
                }
            }
        }

        // Route the container's declared content into its own side table (issue-20261104
        // DM-06/07/08/11/12). This runs **outside** the `children` branch above because the
        // families that need it do not all use `children`: the menu and tool bar declare
        // `menus`/`actions`, and the wizard, accordion and splitter use their own keys.
        if !tab_widget_children && has_routing_key(obj, &child_ids) {
            route_container_content(
                widget_id,
                widget_type,
                obj,
                &child_ids,
                registry,
                binding,
                depth,
                resolver,
                &ancestor_chain(),
            )?;
        }

        // Handle layout inline
        if let Some(layout_val) = obj.get("layout") {
            let kind = parse_layout_kind(layout_val)?;
            let layout = create_layout_from_kind(&kind);

            // Stored before the children below are registered, for the same reason as
            // the `"layout"` pseudo-widget branch: a child added before the layout
            // exists is added to nothing.
            store_layout(widget_id, layout);

            // Process layout children from the layout object
            if let Some(layout_obj) = layout_val.as_object() {
                if let Some(children) = layout_obj.get("children").and_then(|v| v.as_array()) {
                    for child_value in children {
                        if let Some(child_obj) = child_value.as_object() {
                            if child_obj.len() == 1 {
                                let (child_type, child_val) = child_obj.iter().next().unwrap();
                                if child_type.eq_ignore_ascii_case("spacer") {
                                    let stretch = child_val
                                        .as_object()
                                        .and_then(|m| json_u32_at(m, "stretch"))
                                        .unwrap_or(1);
                                    add_spacer_to_layout(stretch, widget_id);
                                    continue;
                                }

                                let child_id = Self::instantiate_node(
                                    child_type,
                                    child_val,
                                    Some(widget_id),
                                    registry,
                                    binding,
                                    depth + 1,
                                    resolver,
                                    &ancestor_chain(),
                                )?;

                                if child_id != 0 {
                                    let attrs = ChildLayoutAttrs::from_value(child_val);
                                    // See the note on the other `add_widget_to_layout`
                                    // call: the layout has to be stored before its
                                    // children are registered, or they are added to
                                    // nothing.
                                    add_widget_to_layout(child_id, attrs.stretch, widget_id);
                                    Self::apply_child_placement(
                                        child_id, &attrs, child_type, widget_id, binding,
                                    );
                                }
                            }
                        }
                    }
                }
            }

            apply_layout(widget_id, json_geometry(obj));
        }

        Ok(widget_id)
    }

    /// Create a widget box from a type name and property object.
    ///
    /// # One property route (BIND-05)
    ///
    /// An arm here supplies **only** what the constructor needs: the document's `text`/`title`,
    /// an `items` array, and the handful of keys listed by [`is_loader_owned_key`] as
    /// construction-time scalars whose spelling differs from the control's property name
    /// (`tristate`, `password`, `word_wrap`, `tab_shape`, `h_policy`, `v_policy`, `alpha`). Every
    /// other key is applied once, by the name-driven pass in
    /// [`apply_properties_with_icon_resolver`].
    ///
    /// # Why the arms used to repeat the setters, and why they no longer do
    ///
    /// The table predates the property contract: each arm applied its own scalars, and the
    /// name-driven pass then applied the same keys a second time. That was the "dual route" this
    /// issue filed — two implementations of one property, which agree only until one is edited,
    /// and a ~570-line table in which a new control could silently land on the factory fallback
    /// with different constructor arguments. The arms now carry construction only, so the fallback
    /// and the table produce the same state through the same pass; the tests below pin both halves
    /// of that (an arm may read no published property, and every factory name is constructible).
    pub(crate) fn create_widget(
        widget_type: &str,
        obj: &serde_json::Map<String, Value>,
    ) -> Result<Box<dyn Widget>, String> {
        let geometry = Rect::new(0, 0, 100, 100);
        match lowered(widget_type).as_ref() {
            "window" => {
                let title = obj.get("title").and_then(|v| v.as_str()).unwrap_or("Window");
                Ok(Box::new(Window::new(title.to_string(), geometry)))
            }
            "button" => {
                let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(Box::new(Button::new(text.to_string(), geometry)))
            }
            "label" => {
                let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(Box::new(Label::new(text.to_string(), geometry)))
            }
            "checkbox" => {
                let mut cb = CheckBox::new(geometry);
                // `tristate` is loader-owned: it is a construction-time switch with no published
                // property, so this arm is the only place that can apply it.
                if let Some(tri) = obj.get("tristate").and_then(|v| v.as_bool()) {
                    cb.set_tristate_enabled(tri);
                }
                Ok(Box::new(cb))
            }
            "radiobutton" => Ok(Box::new(RadioButton::new(geometry))),
            "lineedit" => {
                let mut le = LineEdit::new(geometry);
                // `password` is loader-owned: the document states a boolean, and the arm maps it
                // onto `EchoMode` (the control publishes no `password` property).
                if obj.get("password").and_then(|v| v.as_bool()) == Some(true) {
                    le.set_echo_mode(crate::widget::EchoMode::Password);
                }
                Ok(Box::new(le))
            }
            #[cfg(not(alloc_frugal))]
            "textedit" => {
                let mut te = TextEdit::new(geometry);
                // `word_wrap` is loader-owned (the published property is `line_wrap`).
                if let Some(wrap) = obj.get("word_wrap").and_then(|v| v.as_bool()) {
                    te.set_line_wrap(wrap);
                }
                Ok(Box::new(te))
            }
            "combobox" => {
                let mut cb = ComboBox::new(geometry);
                for_each_json_item(obj, |text| cb.add_item(text));
                Ok(Box::new(cb))
            }
            "listbox" => {
                let mut lb = ListBox::new(geometry);
                for_each_json_item(obj, |text| lb.add_item(text));
                Ok(Box::new(lb))
            }
            "slider" => {
                let mut sl = Slider::new(geometry);
                // `minimum` + `maximum` are one atomic range on this control; the scalar property
                // pass writes them one at a time, so the pair is applied here. Every other key is
                // the name-driven pass's.
                apply_range_like(obj, &mut sl);
                Ok(Box::new(sl))
            }
            "scrollbar" => {
                let mut sb = ScrollBar::new(geometry);
                // See `slider`: the minimum/maximum pair is one atomic range.
                apply_range_like(obj, &mut sb);
                Ok(Box::new(sb))
            }
            "progressbar" => {
                let mut pb = ProgressBar::new(geometry);
                // See `slider`: the minimum/maximum pair is one atomic range.
                apply_range_like(obj, &mut pb);
                Ok(Box::new(pb))
            }
            "switch" | "toggle" => Ok(Box::new(Switch::new(geometry))),
            "groupbox" | "panel" => {
                let mut gb = GroupBox::new(geometry);
                if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                    if !title.is_empty() {
                        gb.set_title(title.to_string());
                    }
                }
                Ok(Box::new(gb))
            }
            #[cfg(not(alloc_frugal))]
            "tabwidget" => {
                let mut tw = TabWidget::new(geometry);
                // `tab_shape` is loader-owned: it has no published property (the published
                // `tab_position` is the other axis), so the arm is the only route.
                if let Some(shape) = obj.get("tab_shape").and_then(|v| v.as_str()) {
                    match shape {
                        "triangular" => tw.set_tab_shape(
                            crate::widget::container_widgets::tabwidget::TabShape::Triangular,
                        ),
                        "rectangular" => tw.set_tab_shape(
                            crate::widget::container_widgets::tabwidget::TabShape::Rectangular,
                        ),
                        // Unknown value; use widget default
                        _ => {}
                    }
                }
                Ok(Box::new(tw))
            }
            #[cfg(not(alloc_frugal))]
            "grid" => Ok(Box::new(GridWidget::new(geometry))),
            "spinbox" => Ok(Box::new(SpinBox::new(geometry))),
            #[cfg(not(alloc_frugal))]
            "listview" => Ok(Box::new(ListView::new(geometry))),
            "scrollarea" => {
                let mut sa = ScrollArea::new(geometry);
                // `h_policy` / `v_policy` are loader-owned: the document uses the reference toolkit's policy words
                // and the control publishes the setter names instead.
                if let Some(policy) = obj.get("h_policy").and_then(|v| v.as_str()) {
                    sa.set_horizontal_scroll_bar_policy(match policy {
                        "always_on" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOn
                        }
                        "always_off" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOff
                        }
                        _ => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AsNeeded
                        }
                    });
                }
                if let Some(policy) = obj.get("v_policy").and_then(|v| v.as_str()) {
                    sa.set_vertical_scroll_bar_policy(match policy {
                        "always_on" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOn
                        }
                        "always_off" => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AlwaysOff
                        }
                        _ => {
                            crate::widget::container_widgets::scrollarea::ScrollBarPolicy::AsNeeded
                        }
                    });
                }
                Ok(Box::new(sa))
            }
            "frame" => {
                use crate::widget::base_widgets::frame::Frame;
                Ok(Box::new(Frame::new(geometry)))
            }
            #[cfg(not(alloc_frugal))]
            "messagebox" => {
                let mut mb = MessageBox::new(geometry);
                // `title` and `text` are loader-owned construction arguments; the icon is a
                // published property and is applied by the name-driven pass.
                if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                    if !title.is_empty() {
                        mb.set_title(title.to_string());
                    }
                }
                if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        mb.set_text(text.to_string());
                    }
                }
                Ok(Box::new(mb))
            }
            #[cfg(not(alloc_frugal))]
            "filedialog" => {
                let mut fd = FileDialog::new(geometry);
                // `title` is loader-owned; `mode` and `directory` are published properties.
                if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                    if !title.is_empty() {
                        fd.set_title(title.to_string());
                    }
                }
                Ok(Box::new(fd))
            }
            #[cfg(not(alloc_frugal))]
            "colordialog" => {
                let mut cd = ColorDialog::new(geometry);
                // `alpha` is loader-owned (the published property is `options_alpha`).
                if let Some(alpha) = obj.get("alpha").and_then(|v| v.as_bool()) {
                    cd.set_options_alpha(alpha);
                }
                Ok(Box::new(cd))
            }
            #[cfg(not(alloc_frugal))]
            "fontdialog" => Ok(Box::new(FontDialog::new(geometry))),
            _ => {
                // Not one of the widget types this arm table knows how to
                // configure. Before giving up, ask the capability registry: it
                // holds a constructor for every registered control, so a control
                // added to the library becomes reachable from JSON without an edit
                // here. The registry-built control gets its scalar properties from
                // the name-driven pass in `apply_properties`, which is why this
                // fallback does not need to reproduce any of the setters above.
                //
                // A build without the registry (a stripped profile) answers `None`
                // and the error below is reported, which is the truth there.
                let factory = crate::json::schema_factory();
                // The factory takes the same constructor argument the arm table reads, so a
                // control registered with the factory but not named here starts from the same
                // text rather than from `""`; the property pass then applies the document's own
                // value over that starting point (BIND-05, landed).
                let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(widget) = factory.create(widget_type, geometry, text) {
                    return Ok(widget);
                }
                Err(format!(
                    "unknown widget type '{widget_type}'; it is neither a built-in JSON widget \
                     name nor a name registered with the widget factory"
                ))
            }
        }
    }
}
