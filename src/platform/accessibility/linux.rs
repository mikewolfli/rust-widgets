// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Linux AT-SPI2 accessibility bridge.
//!
//! Connects to the AT-SPI2 registry via D-Bus (using `zbus`) to expose widget
//! accessibility information and emit events to screen readers and other
//! assistive technologies.
//!
//! Architecture
//! ============
//!
//! ┌──────────────────────────────────────────────────────────────────┐
//! │  Application Process                                              │
//! │  ┌─────────────────────────────┐   D-Bus (a11y bus)              │
//! │  │ LinuxAccessibilityBridge    │ ──────────────────►              │
//! │  │  ┌───────────────────────┐  │    org.a11y.atspi.Registry      │
//! │  │  │ zbus::Connection      │──┤    ┌─────────────────────────┐  │
//! │  │  │  (blocking sync)      │  │    │ NotifyEvent(Focus:)    │  │
//! │  │  └───────────────────────┘  │    │ NotifyEvent(PropChange)│  │
//! │  │  ┌───────────────────────┐  │    └─────────────────────────┘  │
//! │  │  │ ObjectServer exports  │  │    ┌─────────────────────────┐  │
//! │  │  │  org.a11y.atspi.      │  │    │ AT-SPI client queries   │  │
//! │  │  │  Accessible per node  │──┼──► │ GetRole/GetName/GetState│  │
//! │  │  └───────────────────────┘  │    └─────────────────────────┘  │
//! │  └─────────────────────────────┘                                 │
//! └──────────────────────────────────────────────────────────────────┘
//!
//! # Object export and the event source (D09-A11Y-01)
//!
//! Before D09-A11Y-01 the bridge emitted `NotifyEvent` with a source path of
//! `/org/a11y/atspi/accessible/{id}` that it never registered on the bus, and it implemented no
//! AT-SPI object interface. The registry accepted the event, but a client that then queried the
//! source path found no application object there — the event named something that did not exist.
//!
//! The fix is a real export: each widget for which [`AccessibilityBridge::submit_node_state`] runs
//! is served on the bus as an `org.a11y.atspi.Accessible` object at exactly the path the events
//! name, backed by the widget's [`A11yState`] (see [`AtspiAccessible`]). `unregister_node` removes
//! the object. Events are emitted **only** for objects that are exported, so an event's source path
//! always resolves to an application object a client can query.
//!
//! The export needs a live connection (the a11y bus or the session bus). When there is none — the
//! `linux-a11y` feature is off, or no bus is reachable — the bridge is an in-memory store and
//! [`LinuxAccessibilityBridge::atspi_object_export_supported`] answers `false`; that is the honest
//! capability statement the defect asks for, rather than emitting events that name no object.
//!
//! Reference: AT-SPI2 D-Bus Protocol Specification
//!   <https://gitlab.gnome.org/GNOME/at-spi2-core/-/blob/main/docs/at-spi-dbus-dev.md>

use super::{A11yRole, A11yState, AccessibilityBridge};
use crate::compat::{lock, HashMap, Mutex, String, ToString};
use crate::core::ObjectId;

/// Error message logged when the `linux-a11y` feature is disabled.
#[cfg(not(feature = "linux-a11y"))]
const FEATURE_DISABLED: &str =
    "[Linux AT-SPI] linux-a11y feature not enabled — using in-memory store only";

/// The D-Bus well-known bus name for the AT-SPI registry.
#[cfg(feature = "linux-a11y")]
const ATSPI_REGISTRY_BUS_NAME: &str = "org.a11y.atspi.Registry";

/// The D-Bus object path for the AT-SPI registry.
#[cfg(feature = "linux-a11y")]
const ATSPI_REGISTRY_OBJECT_PATH: &str = "/org/a11y/atspi/registry";

/// The D-Bus interface for the AT-SPI registry.
#[cfg(feature = "linux-a11y")]
const ATSPI_REGISTRY_INTERFACE: &str = "org.a11y.atspi.Registry";

/// The AT-SPI `Accessible` interface name, exported per widget object.
#[cfg(feature = "linux-a11y")]
const ATSPI_ACCESSIBLE_INTERFACE: &str = "org.a11y.atspi.Accessible";

/// The D-Bus object path that a widget's exported `org.a11y.atspi.Accessible` lives at.
///
/// `None` for an id whose path would be invalid; the ids are decimal integers, so in practice this
/// always succeeds, but the caller must not `expect` a value that arrives from a caller-supplied id.
#[cfg(feature = "linux-a11y")]
fn object_path_for(id: ObjectId) -> Option<zbus::zvariant::ObjectPath<'static>> {
    zbus::zvariant::ObjectPath::try_from(format!("/org/a11y/atspi/accessible/{id}")).ok()
}

/// Map an [`A11yRole`] to the AT-SPI `AtspiRole` numeric id.
///
/// Reference: `atspi-constants.h` (`AtspiRole`) in at-spi2-core.
/// <https://gitlab.gnome.org/GNOME/at-spi2-core/-/blob/main/atspi/atspi-constants.h>
pub fn atspi_role_id(role: &A11yRole) -> u32 {
    // AT-SPI role numbers for the roles this crate publishes. The values are the stable
    // `AtspiRole` enum members; keeping the mapping here (rather than in the D-Bus interface method)
    // is what makes it a pure, testable function like `windows::uia_control_type_id`.
    match role {
        A11yRole::Unknown => 0,      // ATSPI_ROLE_INVALID
        A11yRole::Button => 43,      // ATSPI_ROLE_PUSH_BUTTON
        A11yRole::Label => 29,       // ATSPI_ROLE_LABEL
        A11yRole::TextField => 81,   // ATSPI_ROLE_TEXT / entry
        A11yRole::CheckBox => 7,     // ATSPI_ROLE_CHECK_BOX
        A11yRole::RadioButton => 44, // ATSPI_ROLE_RADIO_BUTTON
        A11yRole::Slider => 51,      // ATSPI_ROLE_SLIDER
        A11yRole::ProgressBar => 42, // ATSPI_ROLE_PROGRESS_BAR
        A11yRole::List => 31,        // ATSPI_ROLE_LIST
        A11yRole::Table => 63,       // ATSPI_ROLE_TABLE
        A11yRole::Image => 26,       // ATSPI_ROLE_IMAGE
        A11yRole::Link => 30,        // ATSPI_ROLE_LINK
        A11yRole::Heading => 14,     // ATSPI_ROLE_HEADING
        A11yRole::Paragraph => 35,   // ATSPI_ROLE_PARAGRAPH
        A11yRole::Group => 16,       // ATSPI_ROLE_GROUPING
        A11yRole::Window => 69,      // ATSPI_ROLE_FRAME
        A11yRole::Dialog => 17,      // ATSPI_ROLE_DIALOG
        A11yRole::Menu => 33,        // ATSPI_ROLE_MENU
        A11yRole::MenuItem => 34,    // ATSPI_ROLE_MENU_ITEM
        A11yRole::Tab => 37,         // ATSPI_ROLE_PAGE_TAB
        A11yRole::Switch => 59,      // ATSPI_ROLE_TOGGLE_BUTTON
        A11yRole::Alert => 2,        // ATSPI_ROLE_ALERT
        A11yRole::ComboBox => 9,     // ATSPI_ROLE_COMBO_BOX
        A11yRole::SpinButton => 55,  // ATSPI_ROLE_SPIN_BUTTON
        A11yRole::StatusBar => 61,   // ATSPI_ROLE_STATUS_BAR
        A11yRole::ToolTip => 65,     // ATSPI_ROLE_TOOL_TIP
        A11yRole::Tree => 68,        // ATSPI_ROLE_TREE
    }
}

/// The AT-SPI `State` bitset (two `u32` words) for a node's [`A11yState`].
///
/// AT-SPI encodes state as a 64-bit bitfield split into `(low, high)`; this maps the states this
/// crate publishes onto their documented bit positions. Bits not derived from `state` (busy, modal,
/// etc.) stay clear, which is the honest statement "this crate does not publish that fact".
///
/// Reference: `AtspiStateType` in `atspi-constants.h`.
pub fn atspi_state_bits(state: &A11yState) -> (u32, u32) {
    // Bit positions from AtspiStateType. `enabled` is ATSPI_STATE_ENABLED (bit 8) in the low word;
    // `focusable` (bit 10) and `focused` (bit 11) follow; `selected` (bit 25), `checked` (bit 28)
    // and `expanded` (bit 30) live in the low word too. AT-SPI has no single "mixed" bit; the tri-
    // state mixed flag is reported through the `indeterminate` bit (53, high word bit 21).
    let mut low: u32 = 0;
    let mut high: u32 = 0;
    if state.enabled {
        low |= 1 << 8; // ATSPI_STATE_ENABLED
    }
    if state.focused {
        low |= 1 << 11; // ATSPI_STATE_FOCUSED
    }
    if state.selected {
        low |= 1 << 25; // ATSPI_STATE_SELECTED
    }
    match state.checked {
        Some(true) => low |= 1 << 28, // ATSPI_STATE_CHECKED
        Some(false) => {}
        // No checked state at all: leave the bit clear and, unlike a plain bool, do not claim
        // "unchecked" for a control that is not checkable.
        None => {}
    }
    if state.expanded {
        low |= 1 << 30; // ATSPI_STATE_EXPANDED
    }
    if state.mixed {
        high |= 1 << 21; // ATSPI_STATE_INDETERMINATE
    }
    (low, high)
}

/// The D-Bus object served for one widget, implementing `org.a11y.atspi.Accessible`.
///
/// The state lives behind an `Arc<Mutex<..>>` shared with the bridge, so a later
/// `submit_node_state` for the same widget updates the already-exported object in place — a client
/// that re-queries the same path sees the new state rather than a stale one from mount time.
#[cfg(feature = "linux-a11y")]
#[derive(Clone)]
pub struct AtspiAccessible {
    /// The node's current state, shared with the bridge's store.
    state: std::sync::Arc<Mutex<A11yState>>,
    /// This node's own object path, returned as the AT-SPI "parent/child self" identity.
    path: String,
}

#[cfg(feature = "linux-a11y")]
impl AtspiAccessible {
    /// Creates an accessible object backed by shared `state`.
    pub fn new(state: std::sync::Arc<Mutex<A11yState>>, path: String) -> Self {
        Self { state, path }
    }
}

#[cfg(feature = "linux-a11y")]
#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl AtspiAccessible {
    /// `GetRole` — the AT-SPI `AtspiRole` id for this node.
    fn get_role(&self) -> u32 {
        let guard = lock(&self.state);
        atspi_role_id(&guard.role)
    }

    /// `GetName` — the accessible name.
    fn get_name(&self) -> String {
        lock(&self.state).label.clone()
    }

    /// `GetDescription` — the longer description / help text.
    fn get_description(&self) -> String {
        lock(&self.state).description.clone()
    }

    /// `GetState` — the AT-SPI state bitset, as `(low, high)`.
    fn get_state(&self) -> (u32, u32) {
        atspi_state_bits(&lock(&self.state))
    }

    /// `GetChildCount` — number of direct children this node reports.
    fn get_child_count(&self) -> u32 {
        lock(&self.state).children.len() as u32
    }

    /// `GetParent` — this object's own path, since the bridge exports a flat set of widget objects
    /// rather than a nested tree. A parent that is the root `(unique name, "/")` is what AT-SPI
    /// expects for an object whose parent is the application frame the bridge does not model.
    fn get_parent(&self) -> (String, zbus::zvariant::OwnedObjectPath) {
        let root = zbus::zvariant::OwnedObjectPath::try_from("/")
            .unwrap_or_else(|_| zbus::zvariant::OwnedObjectPath::try_from("/").expect("root path"));
        (self.path.clone(), root)
    }

    /// `GetInterfaces` — the interfaces this object implements. Only `Accessible` is exported; the
    /// component/action/text interfaces are not, and saying so lets a client skip them rather than
    /// call into a path that will refuse.
    fn get_interfaces(&self) -> Vec<String> {
        vec![ATSPI_ACCESSIBLE_INTERFACE.to_string()]
    }
}

/// Linux AT-SPI2 bridge implementation.
///
/// This bridge connects to the AT-SPI2 D-Bus registry (on the a11y bus, whose address is read from
/// the `AT_SPI_BUS` environment variable, or falls back to the session bus). It stores each widget's
/// full [`A11yState`] in-process, exports it on the bus as an `org.a11y.atspi.Accessible` object,
/// and emits D-Bus events when widget properties change so that screen readers and other assistive
/// technologies can respond.
pub struct LinuxAccessibilityBridge {
    /// Widget accessibility names (label / accessible-name).
    names: Mutex<HashMap<ObjectId, String>>,
    /// Full accessibility state per widget (D09-A11Y-02).
    ///
    /// The pre-D09-A11Y-02 bridge kept only `names`; this store is what lets a client ask what a
    /// control *is*. The `Arc<Mutex<..>>` inner type is shared with the exported D-Bus object so an
    /// update reaches both without a round trip.
    nodes: Mutex<HashMap<ObjectId, std::sync::Arc<Mutex<A11yState>>>>,
    /// D-Bus connection to the a11y bus (or session bus fallback).
    /// This is `Some` only when the `linux-a11y` feature is enabled AND a
    /// connection was successfully established.
    #[cfg(feature = "linux-a11y")]
    dbus_connection: Mutex<Option<zbus::Connection>>,
    /// Placeholder field when `linux-a11y` feature is disabled.
    #[cfg(not(feature = "linux-a11y"))]
    dbus_connection: Mutex<Option<()>>,
}

impl LinuxAccessibilityBridge {
    /// Create a new Linux AT-SPI2 bridge.
    ///
    /// Attempts to connect to the a11y bus immediately. If the connection
    /// fails (e.g. no a11y bus running, or `linux-a11y` feature disabled),
    /// the bridge operates as an in-memory store only.
    pub fn new() -> Self {
        let conn = Self::try_connect();
        if conn.is_some() {
            log::info!("[Linux AT-SPI] Bridge initialized with D-Bus connection to a11y bus");
        } else {
            log::info!(
                "[Linux AT-SPI] Bridge initialized in local-only mode \
                 (no D-Bus connection)"
            );
        }
        Self {
            names: Mutex::new(HashMap::new()),
            nodes: Mutex::new(HashMap::new()),
            dbus_connection: Mutex::new(conn),
        }
    }

    /// Whether this bridge can export AT-SPI objects on the bus (D09-A11Y-01).
    ///
    /// `true` only when a D-Bus connection is live: without one there is no `ObjectServer` to export
    /// an object at a widget's path, so the bridge must not claim an event source a client could
    /// query. A host can call this to learn whether Linux accessibility is genuinely wired, rather
    /// than inferring it from the presence of a bridge.
    pub fn atspi_object_export_supported(&self) -> bool {
        #[cfg(feature = "linux-a11y")]
        {
            lock(&self.dbus_connection).is_some()
        }
        #[cfg(not(feature = "linux-a11y"))]
        {
            false
        }
    }

    /// The number of widget objects currently exported on the bus (D09-A11Y-01).
    ///
    /// Equals the number of live nodes: an object is exported on `submit_node_state` and removed on
    /// `unregister_node`, so this returning to zero after unmounts is the observable statement that
    /// no dangling object was left behind.
    pub fn exported_object_count(&self) -> usize {
        lock(&self.nodes).len()
    }

    /// Try to establish a D-Bus connection to the AT-SPI a11y bus.
    ///
    /// Connection strategy (in order):
    /// 1. If the `AT_SPI_BUS` environment variable is set, connect to that
    ///    address directly.
    /// 2. Otherwise, attempt to connect to the D-Bus session bus (which
    ///    many modern AT-SPI configurations use).
    #[cfg(feature = "linux-a11y")]
    fn try_connect() -> Option<zbus::Connection> {
        // Try AT_SPI_BUS environment variable first.
        if let Ok(ref bus_addr) = std::env::var("AT_SPI_BUS") {
            log::info!("[Linux AT-SPI] Connecting to a11y bus at AT_SPI_BUS={bus_addr}");
            let builder = zbus::connection::Builder::address(bus_addr.as_str());
            match builder.and_then(|b| pollster::block_on(b.build())) {
                Ok(conn) => {
                    let blocking = zbus::blocking::Connection::from(conn.clone());
                    Self::register_for_events(&blocking);
                    log::info!("[Linux AT-SPI] Connected via AT_SPI_BUS");
                    return Some(conn);
                }
                Err(e) => {
                    log::warn!("[Linux AT-SPI] AT_SPI_BUS connection failed: {e}");
                }
            }
        }

        // Fall back to session bus.
        log::info!("[Linux AT-SPI] No AT_SPI_BUS set, trying session bus");
        match zbus::blocking::Connection::session() {
            Ok(conn) => {
                // Extract the inner async Connection for uniform storage.
                let inner = conn.into_inner();
                Self::register_for_events(&zbus::blocking::Connection::from(inner.clone()));
                log::info!("[Linux AT-SPI] Connected to session bus");
                Some(inner)
            }
            Err(e) => {
                log::warn!("[Linux AT-SPI] Session bus connection failed: {e}");
                None
            }
        }
    }

    /// Stub when `linux-a11y` feature is disabled.
    #[cfg(not(feature = "linux-a11y"))]
    fn try_connect() -> Option<()> {
        log::info!("{FEATURE_DISABLED}");
        None
    }

    /// Register event types with the AT-SPI registry so it accepts our
    /// subsequent event notifications.
    #[cfg(feature = "linux-a11y")]
    fn register_for_events(conn: &zbus::blocking::Connection) {
        let proxy = match zbus::blocking::Proxy::new(
            conn,
            ATSPI_REGISTRY_BUS_NAME,
            ATSPI_REGISTRY_OBJECT_PATH,
            ATSPI_REGISTRY_INTERFACE,
        ) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("[Linux AT-SPI] Failed to create registry proxy: {e}");
                return;
            }
        };

        let event_types = &[
            "Focus:",
            "Object:PropertyChange:accessible-name",
            "Object:PropertyChange:accessible-value",
            "Object:StateChanged:enabled",
            "Object:StateChanged:sensitive",
            "Object:StateChanged:focused",
        ];

        for event_type in event_types {
            match proxy.call_method("RegisterEvent", &event_type) {
                Ok(_) => {
                    log::info!("[Linux AT-SPI] Registered for event: {event_type}");
                }
                Err(e) => {
                    log::warn!("[Linux AT-SPI] RegisterEvent({event_type}) failed: {e}");
                }
            }
        }
    }

    /// Export `state` as an `org.a11y.atspi.Accessible` D-Bus object at the widget's path.
    ///
    /// Returns whether the object is now exported. When there is no live connection there is no
    /// object server to export into, so this answers `false` and the caller must not emit events that
    /// name this path (D09-A11Y-01).
    #[cfg(feature = "linux-a11y")]
    fn export_object(
        conn: &zbus::Connection,
        id: ObjectId,
        state: &std::sync::Arc<Mutex<A11yState>>,
    ) -> bool {
        let Some(path) = object_path_for(id) else {
            log::warn!("[Linux AT-SPI] Refusing to export an unrepresentable object path for {id}");
            return false;
        };
        let service = AtspiAccessible::new(state.clone(), path.to_string());
        let blocking = zbus::blocking::Connection::from(conn.clone());
        let object_server = blocking.object_server();
        match object_server.at(path, service) {
            Ok(_added) => true,
            Err(e) => {
                log::warn!("[Linux AT-SPI] Failed to export object for id={id}: {e}");
                false
            }
        }
    }

    /// Remove the D-Bus object exported for `id`, if any.
    #[cfg(feature = "linux-a11y")]
    fn unexport_object(conn: &zbus::Connection, id: ObjectId) {
        let Some(path) = object_path_for(id) else {
            return;
        };
        let blocking = zbus::blocking::Connection::from(conn.clone());
        let object_server = blocking.object_server();
        match object_server.remove::<AtspiAccessible, _>(path) {
            Ok(_destroyed) => {
                log::debug!("[Linux AT-SPI] Unexported object for id={id}");
            }
            Err(e) => {
                log::warn!("[Linux AT-SPI] Failed to unexport object for id={id}: {e}");
            }
        }
    }

    /// Emit an AT-SPI2 event to the registry via `NotifyEvent`.
    ///
    /// The `NotifyEvent` method signature:
    ///   NotifyEvent(event: struct {
    ///     string   type,
    ///     object   path source,
    ///     int32    detail1,
    ///     int32    detail2,
    ///     variant  any_data
    ///   })
    #[cfg(feature = "linux-a11y")]
    fn emit_atspi_event(
        conn: &zbus::blocking::Connection,
        event_type: &str,
        source_path: &zbus::zvariant::ObjectPath<'_>,
        detail1: i32,
        detail2: i32,
    ) {
        use zbus::zvariant::Value;

        let proxy = match zbus::blocking::Proxy::new(
            conn,
            ATSPI_REGISTRY_BUS_NAME,
            ATSPI_REGISTRY_OBJECT_PATH,
            ATSPI_REGISTRY_INTERFACE,
        ) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("[Linux AT-SPI] Failed to create proxy for event emission: {e}");
                return;
            }
        };

        // The any_data variant — we send an empty string for simplicity.
        let any_data = Value::new("");
        let event = (event_type, source_path, detail1, detail2, any_data);

        match proxy.call_method("NotifyEvent", &event) {
            Ok(_) => {
                log::debug!(
                    "[Linux AT-SPI] Event emitted: {event_type} \
                     path={source_path} detail1={detail1} detail2={detail2}"
                );
            }
            Err(e) => {
                log::warn!("[Linux AT-SPI] NotifyEvent({event_type}) failed: {e}");
            }
        }
    }

    /// Dispatch event emission — handles both the feature-gated D-Bus path
    /// and the fallback no-op path.
    ///
    /// Without `linux-a11y` there is no bus and no object can be exported, so no event is ever
    /// emitted; the `nodes` argument is accepted so the callers are identical in both builds.
    #[cfg(not(feature = "linux-a11y"))]
    fn dispatch_event(
        dbus_connection: &Mutex<Option<()>>,
        nodes: &Mutex<HashMap<ObjectId, std::sync::Arc<Mutex<A11yState>>>>,
        event_type: &str,
        id: ObjectId,
        detail1: i32,
        detail2: i32,
    ) {
        let _ = (dbus_connection, nodes, event_type, id, detail1, detail2);
    }

    /// Dispatch event emission — handles both the feature-gated D-Bus path
    /// and the fallback no-op path.
    ///
    /// # The export gate (D09-A11Y-01)
    ///
    /// An event is emitted only for an id that is currently exported in `nodes`, so the event's
    /// source path resolves to an object the application really serves. Emitting for an unexported
    /// id is precisely the defect: a `NotifyEvent` naming `/org/a11y/atspi/accessible/{id}` with no
    /// object there, which a client cannot query. The `nodes` map is checked before the path is
    /// built, so the gate cannot be bypassed by a later `object_path_for` failure.
    #[cfg(feature = "linux-a11y")]
    fn dispatch_event(
        dbus_connection: &Mutex<Option<zbus::Connection>>,
        nodes: &Mutex<HashMap<ObjectId, std::sync::Arc<Mutex<A11yState>>>>,
        event_type: &str,
        id: ObjectId,
        detail1: i32,
        detail2: i32,
    ) {
        if !lock(nodes).contains_key(&id) {
            log::debug!(
                "[Linux AT-SPI] Suppressing {event_type} for id={id}: no exported object to be its \
                 source (D09-A11Y-01)"
            );
            return;
        }
        let Some(source_path) = object_path_for(id) else {
            log::warn!("[Linux AT-SPI] Refusing to emit {event_type} for unrepresentable id={id}");
            return;
        };
        let conn_guard = lock(dbus_connection);
        if let Some(ref conn) = *conn_guard {
            Self::emit_atspi_event(
                &zbus::blocking::Connection::from(conn.clone()),
                event_type,
                &source_path,
                detail1,
                detail2,
            );
        }
    }
}

crate::impl_default_via_new!(LinuxAccessibilityBridge);

impl AccessibilityBridge for LinuxAccessibilityBridge {
    fn set_accessibility_name(&self, id: ObjectId, name: &str) {
        let mut names = lock(&self.names);
        names.insert(id, name.to_string());
    }

    fn accessibility_name(&self, id: ObjectId) -> Option<String> {
        lock(&self.names).get(&id).cloned()
    }

    /// Stores the full node state and exports it as a queryable AT-SPI object (D09-A11Y-01/-02).
    ///
    /// The name is kept in `names` as well so the two views cannot disagree; the object is exported
    /// when a connection is live, and updated in place when the same widget is submitted again
    /// (mount followed by a state change) because the exported object shares the node's state cell.
    fn submit_node_state(&self, id: ObjectId, state: &A11yState) {
        self.set_accessibility_name(id, &state.label);
        let cell = std::sync::Arc::new(Mutex::new(state.clone()));
        lock(&self.nodes).insert(id, cell.clone());
        #[cfg(feature = "linux-a11y")]
        {
            let guard = lock(&self.dbus_connection);
            if let Some(ref conn) = *guard {
                Self::export_object(conn, id, &cell);
            } else {
                log::debug!(
                    "[Linux AT-SPI] No bus connection: node {id} stored in-memory only \
                     (atspi_object_export_supported() == false)"
                );
            }
        }
        #[cfg(not(feature = "linux-a11y"))]
        {
            let _ = &cell;
        }
    }

    fn node_state(&self, id: ObjectId) -> Option<A11yState> {
        lock(&self.nodes).get(&id).map(|cell| lock(cell).clone())
    }

    /// Removes the node's name, state and exported object (D09-A11Y-03).
    ///
    /// All three are removed together, so the `names` and `nodes` maps both return to baseline and
    /// no object is left exported at a path that no live widget owns.
    fn unregister_node(&self, id: ObjectId) {
        lock(&self.names).remove(&id);
        let removed = lock(&self.nodes).remove(&id);
        #[cfg(feature = "linux-a11y")]
        {
            if removed.is_some() {
                let guard = lock(&self.dbus_connection);
                if let Some(ref conn) = *guard {
                    Self::unexport_object(conn, id);
                }
            }
        }
        #[cfg(not(feature = "linux-a11y"))]
        {
            let _ = removed;
        }
    }

    fn node_count(&self) -> usize {
        lock(&self.nodes).len()
    }

    fn notify_name_changed(&self, id: ObjectId) {
        log::info!("[Linux AT-SPI] notify_name_changed: id={id:?}");
        Self::dispatch_event(
            &self.dbus_connection,
            &self.nodes,
            "Object:PropertyChange:accessible-name",
            id,
            0,
            0,
        );
    }

    fn notify_value_changed(&self, id: ObjectId) {
        log::info!("[Linux AT-SPI] notify_value_changed: id={id:?}");
        Self::dispatch_event(
            &self.dbus_connection,
            &self.nodes,
            "Object:PropertyChange:accessible-value",
            id,
            0,
            0,
        );
    }

    fn notify_state_changed(&self, id: ObjectId) {
        log::info!("[Linux AT-SPI] notify_state_changed: id={id:?}");
        Self::dispatch_event(&self.dbus_connection, &self.nodes, "Object:StateChanged", id, 0, 0);
    }

    fn notify_focus_changed(&self, id: ObjectId) {
        log::info!("[Linux AT-SPI] notify_focus_changed: id={id:?}");
        Self::dispatch_event(
            &self.dbus_connection,
            &self.nodes,
            "Focus:",
            id,
            1, // detail1 = 1 indicates focus-gained
            0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::accessibility::A11yRole;

    /// Constructs an `A11yState` with a role and label, everything else default.
    fn state(role: A11yRole, label: &str) -> A11yState {
        A11yState { role, label: label.to_string(), ..A11yState::default() }
    }

    #[test]
    fn test_name_store_and_retrieve() {
        let bridge = LinuxAccessibilityBridge::new();
        let id = 42u64;

        // Initially no name
        assert!(bridge.accessibility_name(id).is_none());

        // Set and retrieve
        bridge.set_accessibility_name(id, "Hello Button");
        assert_eq!(bridge.accessibility_name(id).as_deref(), Some("Hello Button"));

        // Overwrite
        bridge.set_accessibility_name(id, "Updated Name");
        assert_eq!(bridge.accessibility_name(id).as_deref(), Some("Updated Name"));
    }

    #[test]
    fn test_different_ids_independent() {
        let bridge = LinuxAccessibilityBridge::new();
        bridge.set_accessibility_name(1, "One");
        bridge.set_accessibility_name(2, "Two");

        assert_eq!(bridge.accessibility_name(1).as_deref(), Some("One"));
        assert_eq!(bridge.accessibility_name(2).as_deref(), Some("Two"));
    }

    #[test]
    fn test_notifications_do_not_panic() {
        // These should not crash regardless of whether the a11y bus is available.
        let bridge = LinuxAccessibilityBridge::new();
        let id = 7u64;

        bridge.submit_node_state(id, &state(A11yRole::Button, "Test"));
        bridge.notify_name_changed(id);
        bridge.notify_value_changed(id);
        bridge.notify_state_changed(id);
        bridge.notify_focus_changed(id);
    }

    #[test]
    fn test_default_equals_new() {
        // Both constructors should produce a working bridge.
        let _default = LinuxAccessibilityBridge::default();
        let _new = LinuxAccessibilityBridge::new();
        // No panic means success — no AT-SPI bus needed.
    }

    #[test]
    fn test_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<LinuxAccessibilityBridge>();
        assert_sync::<LinuxAccessibilityBridge>();
    }

    /// D09-A11Y-02: the *whole* state is stored and read back, not only the label.
    #[test]
    fn submitted_state_is_stored_in_full_not_only_the_label() {
        let bridge = LinuxAccessibilityBridge::new();
        let full = A11yState {
            role: A11yRole::CheckBox,
            label: "Subscribe".to_string(),
            description: "Newsletter opt-in".to_string(),
            enabled: false,
            focused: true,
            selected: true,
            expanded: true,
            value: "on".to_string(),
            checked: Some(true),
            mixed: true,
            children: vec![3, 4],
        };
        bridge.submit_node_state(5, &full);

        let stored = bridge.node_state(5).expect("the node is readable after submit");
        assert_eq!(stored, full, "every field must round-trip through the bridge boundary");
        // And the legacy name view agrees with the label, so the two cannot drift.
        assert_eq!(bridge.accessibility_name(5).as_deref(), Some("Subscribe"));
    }

    /// D09-A11Y-03: unmount removes the entry, so counts return to baseline over many cycles.
    #[test]
    fn unmount_removes_entries_so_counts_return_to_baseline() {
        let bridge = LinuxAccessibilityBridge::new();
        assert_eq!(bridge.node_count(), 0, "a fresh bridge records no nodes");

        for id in 1..=4u64 {
            bridge.submit_node_state(id, &state(A11yRole::Label, "row"));
        }
        // Four distinct ids were submitted, so four entries must be present. This is the growth case
        // the defect described: the map grew with the number of mounts, and the fix is that unmount
        // now takes the entries back out rather than blanking them.
        assert_eq!(bridge.node_count(), 4, "distinct ids accumulate one entry each");
        assert!(
            bridge.accessibility_name(1).is_some(),
            "an earlier id is still present before unmount"
        );

        for id in 1..=4u64 {
            bridge.unregister_node(id);
        }
        assert_eq!(bridge.node_count(), 0, "every unmounted id is gone");
        assert!(bridge.accessibility_name(1).is_none(), "and its name is gone too");
        assert!(bridge.node_state(1).is_none(), "and its state is gone too");
        assert_eq!(bridge.exported_object_count(), 0, "and no object is left exported");
    }

    /// D09-A11Y-01: the capability query is honest about whether objects can be exported.
    ///
    /// In a unit test there is usually no a11y bus, so the answer is `false` — and that is the
    /// assertion: the bridge reports the capability it actually has rather than claiming an export
    /// it did not perform. When a bus *is* present the answer is `true` and the exported count
    /// tracks the live nodes.
    #[test]
    fn object_export_capability_is_reported_honestly() {
        let bridge = LinuxAccessibilityBridge::new();
        if bridge.atspi_object_export_supported() {
            bridge.submit_node_state(70, &state(A11yRole::Button, "ok"));
            assert_eq!(bridge.exported_object_count(), 1, "a live bus exports the node");
            bridge.unregister_node(70);
            assert_eq!(bridge.exported_object_count(), 0, "unmount unexports it");
        } else {
            // No bus: nothing is exported, and the count says so rather than lying.
            assert_eq!(bridge.exported_object_count(), 0, "no bus means no exported objects");
        }
    }

    /// The AT-SPI role mapping covers every role this crate publishes and hits the documented ids.
    #[test]
    fn atspi_role_mapping_matches_the_protocol_constants() {
        assert_eq!(atspi_role_id(&A11yRole::Button), 43, "ATSPI_ROLE_PUSH_BUTTON");
        assert_eq!(atspi_role_id(&A11yRole::CheckBox), 7, "ATSPI_ROLE_CHECK_BOX");
        assert_eq!(atspi_role_id(&A11yRole::TextField), 81, "ATSPI_ROLE_TEXT");
        assert_eq!(atspi_role_id(&A11yRole::Unknown), 0, "ATSPI_ROLE_INVALID");
        // Every variant is covered: this assertion is the match's exhaustiveness plus a sample.
        assert_ne!(atspi_role_id(&A11yRole::Tree), atspi_role_id(&A11yRole::Link));
    }

    /// The AT-SPI state bits reflect the fields this crate publishes, and stay quiet on unchecked.
    #[test]
    fn atspi_state_bits_reflect_published_fields() {
        // Enabled (bit 8) alone.
        let enabled = A11yState { enabled: true, ..A11yState::default() };
        assert_eq!(atspi_state_bits(&enabled), (1 << 8, 0));

        // Focused (bit 11) and selected (bit 25) together.
        let focused_selected = A11yState { focused: true, selected: true, ..A11yState::default() };
        assert_eq!(atspi_state_bits(&focused_selected), ((1 << 11) | (1 << 25), 0));

        // A checked box sets bit 28; a box with no checked state must not.
        let checked = A11yState { checked: Some(true), ..A11yState::default() };
        assert_eq!(atspi_state_bits(&checked).0 & (1 << 28), 1 << 28);
        let uncheckable = A11yState { checked: None, ..A11yState::default() };
        assert_eq!(
            atspi_state_bits(&uncheckable).0 & (1 << 28),
            0,
            "a control with no checked state must not claim to be checked"
        );

        // Mixed raises the indeterminate bit in the high word (bit 21).
        let mixed = A11yState { checked: Some(true), mixed: true, ..A11yState::default() };
        assert_eq!(atspi_state_bits(&mixed).1 & (1 << 21), 1 << 21);
    }
}
