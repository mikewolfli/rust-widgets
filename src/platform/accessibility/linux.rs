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
//! │  │  │ Widget property store │  │    ┌─────────────────────────┐  │
//! │  │  │  (HashMap<ObjectId,   │  │    │ Screen reader /         │  │
//! │  │  │   name, role, state>) │  │    │ AT-SPI client           │  │
//! │  │  └───────────────────────┘  │    └─────────────────────────┘  │
//! │  └─────────────────────────────┘                                 │
//! └──────────────────────────────────────────────────────────────────┘
//!
//! Reference: AT-SPI2 D-Bus Protocol Specification
//!   <https://gitlab.gnome.org/GNOME/at-spi2-core/-/blob/main/docs/at-spi-dbus-dev.md>

use super::AccessibilityBridge;
use crate::core::ObjectId;
use std::collections::HashMap;
use std::sync::Mutex;

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

/// Linux AT-SPI2 bridge implementation.
///
/// This bridge connects to the AT-SPI2 D-Bus registry (on the a11y bus,
/// whose address is read from the `AT_SPI_BUS` environment variable, or
/// falls back to the session bus). It stores widget accessibility metadata
/// in-process and emits D-Bus signals when widget properties change so that
/// screen readers and other assistive technologies can respond.
pub struct LinuxAccessibilityBridge {
    /// Widget accessibility names (label / accessible-name).
    names: Mutex<HashMap<ObjectId, String>>,
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
        Self { names: Mutex::new(HashMap::new()), dbus_connection: Mutex::new(conn) }
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

    /// Obtain or create an AT-SPI object path for a widget ID.
    #[cfg(feature = "linux-a11y")]
    fn object_path_for(id: ObjectId) -> zbus::zvariant::ObjectPath<'static> {
        zbus::zvariant::ObjectPath::try_from(format!("/org/a11y/atspi/accessible/{id}"))
            .expect("valid AT-SPI accessible object path")
    }

    /// Dispatch event emission — handles both the feature-gated D-Bus path
    /// and the fallback no-op path.
    #[cfg(not(feature = "linux-a11y"))]
    fn dispatch_event(
        dbus_connection: &Mutex<Option<()>>,
        event_type: &str,
        id: ObjectId,
        detail1: i32,
        detail2: i32,
    ) {
        let _ = (dbus_connection, event_type, id, detail1, detail2);
    }

    /// Dispatch event emission — handles both the feature-gated D-Bus path
    /// and the fallback no-op path.
    #[cfg(feature = "linux-a11y")]
    fn dispatch_event(
        dbus_connection: &Mutex<Option<zbus::Connection>>,
        event_type: &str,
        id: ObjectId,
        detail1: i32,
        detail2: i32,
    ) {
        #[cfg(feature = "linux-a11y")]
        {
            let conn_guard = dbus_connection.lock().expect("dbus_connection lock");
            if let Some(ref conn) = *conn_guard {
                let source_path = Self::object_path_for(id);
                Self::emit_atspi_event(
                    &zbus::blocking::Connection::from(conn.clone()),
                    event_type,
                    &source_path,
                    detail1,
                    detail2,
                );
            }
        }
        #[cfg(not(feature = "linux-a11y"))]
        {
            let _ = (dbus_connection, event_type, id, detail1, detail2);
        }
    }
}

impl Default for LinuxAccessibilityBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessibilityBridge for LinuxAccessibilityBridge {
    fn set_accessibility_name(&self, id: ObjectId, name: &str) {
        let mut names = self.names.lock().expect("names lock");
        names.insert(id, name.to_string());
    }

    fn accessibility_name(&self, id: ObjectId) -> Option<String> {
        self.names.lock().expect("names lock").get(&id).cloned()
    }

    fn notify_name_changed(&self, id: ObjectId) {
        log::info!("[Linux AT-SPI] notify_name_changed: id={id:?}");
        Self::dispatch_event(
            &self.dbus_connection,
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
            "Object:PropertyChange:accessible-value",
            id,
            0,
            0,
        );
    }

    fn notify_state_changed(&self, id: ObjectId) {
        log::info!("[Linux AT-SPI] notify_state_changed: id={id:?}");
        Self::dispatch_event(&self.dbus_connection, "Object:StateChanged", id, 0, 0);
    }

    fn notify_focus_changed(&self, id: ObjectId) {
        log::info!("[Linux AT-SPI] notify_focus_changed: id={id:?}");
        Self::dispatch_event(
            &self.dbus_connection,
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

        bridge.set_accessibility_name(id, "Test");
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
}
