//! WASM control gallery: sixteen tab pages, one card per registered control kind.
//!
//! # What the tabs are
//!
//! The window is one `TabWidget` with **sixteen page panels**, one per page of
//! controls. The tab strip selects the page; only the selected panel and its cards
//! are visible, because the painter skips an invisible widget's whole subtree.
//! `Prev`/`Next` move the tab selection, and clicking a tab goes through the same
//! path, so there is one page state rather than two that can disagree.
//!
//! # What the cards are
//!
//! The card list comes from [`WidgetFactory::widget_names`], not from a hand-typed
//! sample, so a control the factory can construct cannot be missing here. Each card
//! is built by the factory and filled by
//! [`sample_fill::apply`](rust_widgets::sample_fill), which gives each control its
//! own content.
//!
//! # Composite controls stay composite
//!
//! A composite is **one** card. `tab_widget` shows its own tabs, `menu_bar` its own
//! menus, `tree_view` its own rows, `splitter` its own panes. The gallery never
//! enumerates a composite's children as separate cards: a child is part of the
//! control that owns it.
//!
//! # Why the demo paints
//!
//! The wasm backend owns the `<canvas>` and forwards pointer and key input into the
//! widget runtime, but it does not blit: its `supports_surfaces` documentation says
//! the host learns what went stale and pulls the RGBA to paint the canvas. This demo
//! therefore runs one `requestAnimationFrame` loop that advances the library with
//! [`drive_frame`](rust_widgets::drive_frame) and blits
//! [`render_window_frame`](rust_widgets::widget::runtime::render_window_frame)
//! whenever the frame owes pixels.

use std::sync::{Arc, Mutex, MutexGuard};

use rust_widgets::app::{
    App, AppConfig, ButtonHandle, LabelHandle, PanelHandle, SurfaceHandle, TabWidgetHandle,
    WidgetHandle, WindowHandle,
};
use rust_widgets::core::{Color, ObjectId, Rect, Size};
use rust_widgets::theme::global_theme_manager;
use rust_widgets::widget::base_widgets::button::{Button, IconPosition};
use rust_widgets::widget::container_widgets::tabwidget::TabWidget;
use rust_widgets::{IconName, IconSource, WidgetFactory};

/// The `<canvas>` id the wasm backend binds to (`WasmPlatform::new`'s default).
pub const CANVAS_ID: &str = "wgpu-canvas";
/// Window title and page heading.
pub const WINDOW_TITLE: &str = "rust_widgets — control gallery";

/// Logical window size; `index.html` sizes the canvas to the same rectangle.
pub const WIDTH: u32 = 1040;
pub const HEIGHT: u32 = 760;

/// Grid: four columns of `CARD_WIDTH`, four rows of `CARD_HEIGHT`.
const COLUMNS: usize = 4;
const CARD_WIDTH: u32 = 240;
/// Caption plus the control. The control's own rectangle is 240 × 120, which is the
/// size `widget::census` constructs every control at.
const CARD_HEIGHT: u32 = 140;
const CAPTION_HEIGHT: u32 = 20;
const GAP: u32 = 14;
/// Band the paging buttons live in.
const HEADER_HEIGHT: u32 = 58;
/// First card row, below the tab strip the `TabWidget` draws.
const CONTENT_TOP: u32 = 100;
/// Cards per page: the grid above.
const PER_PAGE: usize = COLUMNS * 4;

const PREV_RECT: Rect = Rect { x: GAP as i32, y: 12, width: 104, height: 30 };
const NEXT_RECT: Rect = Rect { x: 126, y: 12, width: 104, height: 30 };
const REPORT_RECT: Rect = Rect { x: 240, y: 18, width: 780, height: 18 };
/// The tab control: its strip sits at `y`, its page area holds the cards below it.
const TABS_RECT: Rect = Rect { x: GAP as i32, y: HEADER_HEIGHT as i32, width: 1012, height: 654 };

/// What the gallery contains; the preview binary prints it.
#[derive(Debug, Clone)]
pub struct PageReport {
    /// Every constructible kind the factory enumerates.
    pub kinds: usize,
    /// Pages those kinds span at [`PER_PAGE`] per page.
    pub pages: usize,
    /// Cards mounted across every page.
    pub mounted: usize,
    /// Kinds the factory could not build at this size.
    pub unbuilt: usize,
    /// Built widgets the backend refused to mount.
    pub unmounted: usize,
    /// The first refusal, as `kind: error`, so the page names the cause.
    pub first_error: Option<String>,
}

/// The demo's state: which page is shown, and every page's cards.
struct Gallery {
    window: WindowHandle,
    /// The page selector. Its pages are the panels, so the widget owns the
    /// "one page at a time" rule rather than this struct re-implementing it.
    tabs: TabWidgetHandle,
    /// Every constructible kind, sorted so a page is stable between runs.
    names: Vec<&'static str>,
    page: usize,
    /// One caption per grid slot, reused across pages by rewriting its text.
    captions: Vec<LabelHandle>,
    /// The page panels, in tab order.
    _panels: Vec<PanelHandle>,
    /// Cards mounted for the page that is shown. Only one page is alive at a time:
    /// the library's per-child layer cache paints a correct frame with a page of
    /// cards, and mixing 242 live controls under one window is what a gallery does
    /// not need (and what made every card paint another card's pixels).
    cards: Vec<SurfaceHandle>,
    /// Labels `compose_container` mounted inside container cards on this page.
    ///
    /// They are registered widgets in their own right, and the runtime does not walk a
    /// child list on teardown, so the gallery unregisters them when the page goes.
    sample_children: Vec<ObjectId>,
    page_label: LabelHandle,
    unbuilt: usize,
    unmounted: usize,
    first_error: Option<String>,
    /// Button and tab activations seen, so the status line can show that input
    /// reaches the demo at all.
    activations: usize,
    /// The last interaction a card reported, e.g. `Button · click`.
    ///
    /// A gallery whose cards only respond visually leaves "is this control alive?" to
    /// the eye; one line naming the control and the fact makes it an observation. Every
    /// card is subscribed to the generic click channel, so this covers every control
    /// that publishes `clicked` without a table per control.
    last_event: Option<String>,
}

impl Gallery {
    fn new(window: &WindowHandle, tabs: TabWidgetHandle) -> Self {
        let mut names: Vec<&'static str> = WidgetFactory::shared()
            .widget_names()
            .into_iter()
            .filter(|name| !name.is_empty())
            .collect();
        // `widget_names` walks a map, so sorting is what makes page 3 be the same
        // page 3 on the next load.
        names.sort_unstable();
        names.dedup();

        let captions = (0..PER_PAGE)
            .map(|slot| {
                let rect = card_rect(slot);
                window.new_label("", rect.x, rect.y, rect.width, CAPTION_HEIGHT)
            })
            .collect();

        Self {
            window: window.clone(),
            tabs,
            names,
            page: 0,
            captions,
            _panels: Vec::new(),
            cards: Vec::new(),
            sample_children: Vec::new(),
            page_label: window.new_label(
                "",
                REPORT_RECT.x,
                REPORT_RECT.y,
                REPORT_RECT.width,
                REPORT_RECT.height,
            ),
            unbuilt: 0,
            unmounted: 0,
            first_error: None,
            activations: 0,
            last_event: None,
        }
    }

    fn page_count(&self) -> usize {
        self.names.len().max(1).div_ceil(PER_PAGE)
    }

    /// The tab widget's id, so a caller outside this struct can select a page the way
    /// the UI does (the preview binary does exactly that).
    fn tabs_id(&self) -> ObjectId {
        self.tabs.raw_id()
    }

    /// Creates the page panels and their tabs.
    ///
    /// One panel per page, added to the tab widget so it re-parents it under the strip
    /// and owns which page is presented. The cards are built per visit in
    /// [`Self::show_page`].
    fn build_pages(&mut self) {
        let pages = self.page_count();
        for page in 0..pages {
            let panel = self.window.new_panel(
                TABS_RECT.x,
                TABS_RECT.y + 32,
                TABS_RECT.width,
                TABS_RECT.height - 32,
            );
            self.tabs.add_tab(&(page + 1).to_string(), Some(panel.raw_id()));
            self._panels.push(panel);
        }
    }

    /// Shows `index`: unmounts the previous page's cards, then builds this page's.
    ///
    /// Re-entrant on purpose: the tab signal and the button path can both land here,
    /// and rebuilding the same page yields the same page. It must not guard on
    /// "already shown" — the first call is what fills the captions.
    ///
    /// `owner` is the shared gallery, so a card's click can report itself into the status
    /// line. It is only captured by the callbacks, never locked here: this function is
    /// called **from** a callback that already holds the lock (the tab's
    /// `current_changed`), and taking it again would deadlock a non-reentrant mutex.
    fn show_page_inner(&mut self, index: usize, owner: &Arc<Mutex<Gallery>>) {
        let page = index.min(self.page_count().saturating_sub(1));
        // The labels the previous page's containers owned go first: they belong to those cards.
        for child in self.sample_children.drain(..) {
            rust_widgets::widget::runtime::unregister(child);
        }
        for surface in self.cards.drain(..) {
            let id = surface.raw_id();
            // A backend that adopts widgets into the window tree leaves the id in the
            // parent's child list; unlinking before the registry entry goes keeps the
            // painter from walking ids that address nothing.
            let _ = rust_widgets::widget::runtime::with_widget_mut(self.window.raw_id(), |host| {
                host.remove_child(id)
            });
            surface.unmount();
        }
        self.page = page;
        self.unbuilt = 0;
        self.unmounted = 0;
        self.first_error = None;
        // The event named the card that was on the page just left, so it no longer describes
        // anything the user can click.
        self.last_event = None;

        let start = page * PER_PAGE;
        for (slot, caption) in self.captions.iter().enumerate() {
            match self.names.get(start + slot) {
                Some(name) => caption.set_text(&pretty_name(name)),
                None => caption.set_text(""),
            }
        }

        let page_names: Vec<&'static str> =
            self.names.iter().skip(start).take(PER_PAGE).copied().collect();
        for (slot, name) in page_names.iter().enumerate() {
            let rect = control_rect(card_rect(slot));
            let mut widget = match WidgetFactory::shared().create(name, rect, &pretty_name(name)) {
                Some(widget) => widget,
                None => {
                    self.unbuilt += 1;
                    self.first_error.get_or_insert_with(|| format!("{name}: unbuildable"));
                    continue;
                }
            };
            // Content before mounting: the control paints what it holds on the first
            // frame, so an empty composite would stay empty.
            rust_widgets::sample_fill::apply(name, widget.as_mut());
            // A container's content is a mounted child, not a value: `apply` alone leaves a
            // scroll area, a stacked widget, a splitter and an MDI area blank.
            let composed =
                rust_widgets::widget::sample_fill::compose_container(name, widget.as_mut());
            match self.window.mount_surface(widget, rect) {
                Ok(surface) => {
                    // The card reports its own click. This is the generic channel rather
                    // than a per-control table: a control that publishes `clicked` is
                    // covered, and one that does not still reacts through its own event
                    // handler — the gallery does not need to know which is which.
                    let owner = Arc::clone(owner);
                    let caption = pretty_name(name);
                    surface.on_click(move || {
                        let mut gallery = lock(&owner);
                        gallery.activations += 1;
                        gallery.last_event = Some(format!("{caption} · click"));
                        gallery.publish_status();
                    });
                    self.cards.push(surface);
                    self.sample_children.extend(composed);
                }
                Err(error) => {
                    // The card never reached the tree, so its composed children have no owner.
                    for child in composed {
                        rust_widgets::widget::runtime::unregister(child);
                    }
                    self.unmounted += 1;
                    self.first_error.get_or_insert_with(|| format!("{name}: {error:?}"));
                }
            }
        }

        self.page_label.set_text(&self.page_text());
        self.publish_status();
        // A page switch replaces every card, so the window's cached frame must go: the
        // incremental painter only repaints damage, and the rectangles the previous
        // page's controls occupied are no longer damage because nothing reports them.
        // Dropping the cache is what turns "new cards painted over the old picture"
        // into a clean page.
        rust_widgets::widget::runtime::forget_cached_frame(self.window.raw_id());
        rust_widgets::widget::runtime::request_repaint_subtree(self.window.raw_id());
        paint(self.window.raw_id());
    }

    /// Publishes what the gallery built: the browser writes it into `#status`, a
    /// host build prints it. Without it a mounting failure is an empty canvas with
    /// no explanation, which is exactly the shape of bug this demo exists to make
    /// visible.
    fn publish_status(&self) {
        let report = self.report();
        let offset = self.page * PER_PAGE;
        let on_page = PER_PAGE.min(report.kinds.saturating_sub(offset));
        status(&format!(
            "{} · page {}/{} · {} kinds · mounted {}/{} · unbuilt {} · unmounted {} · activations {} · {} · {}",
            rust_widgets::backend_name(),
            self.page + 1,
            report.pages,
            report.kinds,
            self.cards.len(),
            on_page,
            report.unbuilt,
            report.unmounted,
            self.activations,
            self.last_event.as_deref().unwrap_or("click a card"),
            report.first_error.as_deref().unwrap_or("no refusals")
        ));
    }

    fn report(&self) -> PageReport {
        PageReport {
            kinds: self.names.len(),
            pages: self.page_count(),
            mounted: self.cards.len(),
            unbuilt: self.unbuilt,
            unmounted: self.unmounted,
            first_error: self.first_error.clone(),
        }
    }

    fn page_text(&self) -> String {
        let mut text = format!(
            "{} kinds · one card per kind · each composite shown as a composite",
            self.names.len()
        );
        if self.unbuilt + self.unmounted > 0 {
            text.push_str(&format!(" · {} unbuilt, {} unmounted", self.unbuilt, self.unmounted));
        }
        text
    }
}

/// Shows `index` under the gallery's own lock.
///
/// Every path that wants a page goes through this or through
/// [`Gallery::show_page_inner`] with the lock already held, so there is no second way to
/// mount a page that could forget the card callbacks or the frame reset.
fn show_page(gallery: &Arc<Mutex<Gallery>>, index: usize) {
    let owner = Arc::clone(gallery);
    lock(gallery).show_page_inner(index, &owner);
}

/// The grid cell a slot occupies.
fn card_rect(slot: usize) -> Rect {
    let column = (slot % COLUMNS) as u32;
    let row = (slot / COLUMNS) as u32;
    Rect::new(
        (GAP + column * (CARD_WIDTH + GAP)) as i32,
        (CONTENT_TOP + row * (CARD_HEIGHT + GAP)) as i32,
        CARD_WIDTH,
        CARD_HEIGHT,
    )
}

/// The rectangle the control itself occupies inside its card.
fn control_rect(card: Rect) -> Rect {
    Rect::new(card.x, card.y + CAPTION_HEIGHT as i32, card.width, card.height - CAPTION_HEIGHT)
}

/// Turns a capability name into a caption: `tab_widget` → `Tab widget`.
fn pretty_name(name: &str) -> String {
    let mut text = String::with_capacity(name.len());
    let mut capitalize = true;
    for character in name.chars() {
        match character {
            '_' => {
                text.push(' ');
                capitalize = true;
            }
            _ if capitalize => {
                text.extend(character.to_uppercase());
                capitalize = false;
            }
            _ => text.push(character),
        }
    }
    text
}

/// The page's background: the active theme's own, so a control that paints
/// nothing still shows the surface it would really sit on.
fn background() -> Color {
    global_theme_manager()
        .current_theme()
        .map(|theme| theme.colors.background)
        .unwrap_or(Color::WHITE)
}

/// Paints the window once.
///
/// On a browser target the frame goes to the canvas; on a host build the frame is
/// computed and dropped, which is what keeps the render path covered by the
/// crate's own `cargo check` for this demo.
fn paint(window: ObjectId) {
    let Some(frame) = rust_widgets::widget::runtime::render_window_frame(
        window,
        Size::new(WIDTH, HEIGHT),
        background(),
    ) else {
        return;
    };
    #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
    host::blit(&frame);
    #[cfg(not(all(target_arch = "wasm32", not(target_os = "wasi"))))]
    let _ = frame;
}

fn lock(gallery: &Arc<Mutex<Gallery>>) -> MutexGuard<'_, Gallery> {
    gallery.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Writes one diagnostic line: into the page on a browser target, to stderr on a
/// host build.
fn status(text: &str) {
    #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
    host::status(text);
    #[cfg(not(all(target_arch = "wasm32", not(target_os = "wasi"))))]
    eprintln!("[gallery] {text}");
}

/// Installs the browser panic reporter; a no-op on a host build.
///
/// Called before anything else can fail, so a panic during start-up is shown in
/// the page instead of aborting into a blank canvas.
pub fn install_panic_hook() {
    #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
    host::install_panic_hook();
}

/// Marks the selected tab with the theme's accent rule.
fn set_document_mode(tabs: &TabWidgetHandle) {
    let _ = rust_widgets::widget::runtime::with_widget_mut(tabs.raw_id(), |widget| {
        let Some(tabs) = (widget as &mut dyn std::any::Any).downcast_mut::<TabWidget>() else {
            return false;
        };
        tabs.set_document_mode(true);
        true
    });
}

/// Puts a library icon in a button's icon slot.
///
/// The bundled text faces carry no arrow glyphs — `←`, `→`, `‹`, `›` all paint as
/// tofu boxes, which is the same reason the library's own pagination draws
/// `ArrowLeft`/`ArrowRight` **outlines** instead of text arrows. The button's icon
/// slot is the supported way to show one.
fn set_button_icon(handle: &ButtonHandle, icon: IconName, position: IconPosition) {
    let _ = rust_widgets::widget::runtime::with_widget_mut(handle.raw_id(), |widget| {
        let Some(button) = (widget as &mut dyn std::any::Any).downcast_mut::<Button>() else {
            return false;
        };
        button.set_icon_source(Some(IconSource::builtin(icon)));
        button.set_icon_position(position);
        button.set_icon_size(18);
        true
    });
}

/// Moves the selection by `delta` pages.
///
/// The gallery lock is **not** held while the tab widget changes: selecting a tab
/// emits `current_changed` synchronously, and that handler takes the same lock to
/// show the page. Holding it here would deadlock a non-reentrant mutex.
fn step(gallery: &Arc<Mutex<Gallery>>, tabs: &TabWidgetHandle, delta: isize) {
    let (current, pages) = {
        let mut gallery = lock(gallery);
        gallery.activations += 1;
        (gallery.page, gallery.page_count())
    };
    let last = pages.saturating_sub(1);
    let target = (current as isize + delta).clamp(0, last as isize) as usize;
    if target == current {
        return;
    }
    // The signal handler shows the page. `show_page` is idempotent, so the fallback
    // below only runs when the tab widget refused the index.
    if !tabs.set_current_index(target) {
        show_page(gallery, target);
    }
}

/// Builds the tab control, the pages and the paging buttons.
fn build_ui(window: &WindowHandle) -> (Arc<Mutex<Gallery>>, PageReport) {
    let tabs = window.new_tab_widget(TABS_RECT.x, TABS_RECT.y, TABS_RECT.width, TABS_RECT.height);
    // `create_tab_widget` seeds "Tab 1" and "Tab 2"; the strip's tabs are our page
    // numbers, so the two placeholders go before the real ones are added.
    tabs.remove_tab(0);
    tabs.remove_tab(0);
    // Document mode is what marks the **current** tab: the strip becomes one band and
    // the selected tab carries a rule in the theme's accent colour. In the default
    // shape the current tab differs from its neighbours only by a 6% fill, which at
    // this size is invisible — the reason "which page am I on" had no answer.
    set_document_mode(&tabs);
    let gallery = Arc::new(Mutex::new(Gallery::new(window, tabs.clone())));

    lock(&gallery).build_pages();

    let previous =
        window.new_button("Prev", PREV_RECT.x, PREV_RECT.y, PREV_RECT.width, PREV_RECT.height);
    set_button_icon(&previous, IconName::ArrowLeft, IconPosition::Leading);
    {
        let gallery = Arc::clone(&gallery);
        let tabs = tabs.clone();
        previous.on_click(move || step(&gallery, &tabs, -1));
    }
    let next =
        window.new_button("Next", NEXT_RECT.x, NEXT_RECT.y, NEXT_RECT.width, NEXT_RECT.height);
    set_button_icon(&next, IconName::ArrowRight, IconPosition::Trailing);
    {
        let gallery = Arc::clone(&gallery);
        let tabs = tabs.clone();
        next.on_click(move || step(&gallery, &tabs, 1));
    }

    // A click on the strip selects that page. This handler is invoked from inside
    // the tab widget's own `set_current_index`, so it must not be reached while the
    // gallery lock is held (see `step`).
    let tabs_id = tabs.raw_id();
    {
        let owner = Arc::clone(&gallery);
        let _ = rust_widgets::widget::runtime::with_widget_mut(tabs_id, |widget| {
            if let Some(tabs) = (widget as &mut dyn std::any::Any).downcast_mut::<TabWidget>() {
                tabs.current_changed.connect(move |index| {
                    let mut gallery = lock(&owner);
                    gallery.activations += 1;
                    gallery.show_page_inner(*index, &owner);
                });
                true
            } else {
                false
            }
        });
    }

    let report = {
        let owner = Arc::clone(&gallery);
        let mut gallery = lock(&owner);
        gallery.show_page_inner(0, &owner);
        gallery.report()
    };
    (gallery, report)
}

/// Creates the window and builds its gallery.
///
/// Shared by [`run`], [`render_offscreen`] and the input-path test, so the picture a
/// preview renders is the picture the browser paints rather than a second
/// construction that could drift from it.
pub fn build_window(app: &mut App) -> (WindowHandle, PageReport) {
    let window = app.new_window(WINDOW_TITLE, 0, 0, WIDTH, HEIGHT);
    // The button and tab callbacks hold the only long-lived references, so the
    // gallery's state survives this function.
    let (_gallery, report) = build_ui(&window);
    (window, report)
}

/// Renders one page offscreen and returns its RGBA pixels.
///
/// This is the host-side composition check: `render_window_frame` is the same entry
/// the browser paint loop blits, and it touches no platform surface. `page` is
/// zero-based; an index past the end shows the last page, matching `show_page`.
pub fn render_offscreen(page: usize) -> Option<Vec<u8>> {
    let mut app = App::with_config(
        AppConfig::default()
            .with_app_name("rust_widgets gallery preview")
            .with_organization("rust_widgets"),
    );
    app.init();
    let window = app.new_window(WINDOW_TITLE, 0, 0, WIDTH, HEIGHT);
    let (gallery, report) = build_ui(&window);
    // Switch pages the way the UI does: select the tab, whose `current_changed` shows
    // the page. Calling `show_page` directly would leave the strip pointing at page 1
    // and prove a path the browser never takes.
    if page != 0 {
        let tabs_id = lock(&gallery).tabs_id();
        let _ = rust_widgets::widget::runtime::with_widget_mut(tabs_id, |widget| {
            if let Some(tabs) = (widget as &mut dyn std::any::Any).downcast_mut::<TabWidget>() {
                tabs.set_current_index(page);
                true
            } else {
                false
            }
        });
    }
    let frame = rust_widgets::widget::runtime::render_window_frame(
        window.raw_id(),
        Size::new(WIDTH, HEIGHT),
        background(),
    );
    eprintln!(
        "[preview] page={} kinds={} pages={} mounted={} unbuilt={} unmounted={} first_error={:?} frame={:?}",
        page + 1,
        report.kinds,
        report.pages,
        report.mounted,
        report.unbuilt,
        report.unmounted,
        report.first_error,
        frame.as_ref().map(Vec::len)
    );
    frame
}

/// Starts the demo.
///
/// `main` calls this, and so does the browser's `#[wasm_bindgen(start)]` export: one
/// construction path for both targets.
pub fn run() {
    install_panic_hook();

    let mut app = App::with_config(
        AppConfig::default()
            .with_app_name("rust_widgets gallery")
            .with_organization("rust_widgets"),
    );
    app.init();

    let window = app.new_window(WINDOW_TITLE, 0, 0, WIDTH, HEIGHT);
    let gallery = build_ui(&window);

    #[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
    {
        let _ = &gallery;
        host::start(window.raw_id());
    }

    #[cfg(not(all(target_arch = "wasm32", not(target_os = "wasi"))))]
    {
        let _ = &gallery;
        app.run();
    }
}

/// Browser glue: the frame loop and the canvas blit.
///
/// Kept in its own module so every browser type is named in exactly one place; a
/// host build compiles none of it.
#[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
mod host {
    use std::cell::RefCell;
    use std::rc::Rc;

    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    use super::{paint, CANVAS_ID, HEIGHT, WIDTH};

    /// The loop's callback: shared with the browser between frames, and owned by the
    /// thread-local below so dropping it does not kill the loop.
    type FrameCallback = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

    thread_local! {
        /// Holds the loop's closure for the life of the page. A dropped closure is a
        /// dead `requestAnimationFrame` callback.
        static FRAME_LOOP: RefCell<Option<FrameCallback>> = const { RefCell::new(None) };
    }

    fn canvas() -> Option<web_sys::HtmlCanvasElement> {
        let document = web_sys::window()?.document()?;
        document.get_element_by_id(CANVAS_ID)?.dyn_into::<web_sys::HtmlCanvasElement>().ok()
    }

    fn context() -> Option<web_sys::CanvasRenderingContext2d> {
        canvas()?.get_context("2d").ok().flatten()?.dyn_into().ok()
    }

    /// Writes the gallery's report into the page's `#status` element.
    pub(super) fn status(text: &str) {
        if let Some(node) = web_sys::window()
            .and_then(|page| page.document())
            .and_then(|document| document.get_element_by_id("status"))
        {
            node.set_text_content(Some(text));
        }
    }

    /// Shows a panic in the page instead of leaving a blank canvas.
    ///
    /// A `wasm32-unknown-unknown` panic aborts with a bare `unreachable`, which the
    /// browser reports as `RuntimeError: unreachable executed` and nothing else. The
    /// hook writes the message the panic actually carried into `#error`, so a failure
    /// here is readable rather than a silent empty window.
    pub(super) fn install_panic_hook() {
        std::panic::set_hook(Box::new(|info| {
            let text = format!("panicked: {info}");
            web_sys::console::error_1(&text.clone().into());
            let Some(document) = web_sys::window().and_then(|page| page.document()) else {
                return;
            };
            if let Some(node) = document.get_element_by_id("error") {
                node.set_text_content(Some(&text));
                if let Some(element) = node.dyn_ref::<web_sys::HtmlElement>() {
                    let _ = element.style().set_property("display", "block");
                }
            }
        }));
    }

    /// Copies one RGBA frame into the canvas.
    pub(super) fn blit(frame: &[u8]) {
        let Some(context) = context() else {
            return;
        };
        let Ok(image) = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(frame),
            WIDTH,
            HEIGHT,
        ) else {
            return;
        };
        let _ = context.put_image_data(&image, 0.0, 0.0);
    }

    /// Installs the frame loop: one `requestAnimationFrame` per frame.
    pub(super) fn start(window: u64) {
        let holder: FrameCallback = Rc::new(RefCell::new(None));
        let holder_for_callback = Rc::clone(&holder);
        *holder.borrow_mut() = Some(Closure::<dyn FnMut()>::new(move || {
            let outcome = rust_widgets::drive_frame(16);
            // A frame owes pixels when it drained input, advanced an animation or had
            // a repaint requested; a still gallery otherwise costs nothing.
            let due = outcome.repaints_submitted > 0
                || outcome.events_dispatched > 0
                || outcome.controls_ticked > 0
                || outcome.needs_another_frame;
            if due {
                paint(window);
            }
            if let Some(callback) = holder_for_callback.borrow().as_ref() {
                let function: &js_sys::Function = callback.as_ref().unchecked_ref();
                if let Some(page) = web_sys::window() {
                    let _ = page.request_animation_frame(function);
                }
            }
        }));
        if let Some(callback) = holder.borrow().as_ref() {
            let function: &js_sys::Function = callback.as_ref().unchecked_ref();
            if let Some(page) = web_sys::window() {
                let _ = page.request_animation_frame(function);
            }
        }
        FRAME_LOOP.with(|slot| *slot.borrow_mut() = Some(holder));
    }
}
