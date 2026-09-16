// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::HashMap;
use core::any::Any;
/// Numeric handle identifying a plugin inside a [`PluginManager`].
///
/// Ids are assigned by [`PluginManager::register`], start at 1 and increase by one
/// per registration; they are never reused within a manager instance, because
/// `clear` intentionally leaves the counter untouched.
pub type PluginId = u64;
/// Handler type for content transformation plugins.
pub type ContentHandler = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Lifecycle state of a registered plugin.
///
/// The states are not ordered numerically; the discriminants only exist to make
/// the variants distinct. [`PluginManager`] only allows a plugin to be enabled
/// from [`PluginState::Installed`] or [`PluginState::Disabled`].
pub enum PluginState {
    /// Known to the manager but not yet installed; the initial state of a freshly
    /// constructed plugin, before `register` overwrites it.
    NotInstalled,
    /// Registered successfully; `Plugin::on_load` has completed.
    Installed,
    /// Installed and active; it participates in `broadcast` and accepts messages.
    Enabled,
    /// Installed but inactive; messages sent to it are dropped.
    Disabled,
    /// Refused or withdrawn by the host; the manager will not enable it.
    Blocked,
    /// Failed or otherwise faulted.
    Error,
}
#[derive(Debug, Clone)]
/// Descriptive metadata for a plugin.
pub struct PluginInfo {
    /// Manager-assigned handle; 0 until the plugin is registered.
    pub id: PluginId,
    /// Display name of the plugin.
    pub name: String,
    /// Version string, in whatever form the plugin author chose.
    pub version: String,
    /// Human-readable summary of what the plugin does.
    pub description: String,
    /// Name of the plugin author or vendor.
    pub author: String,
    /// Project or documentation URL; `None` when the plugin does not publish one.
    pub homepage: Option<String>,
    /// Permissions this plugin declares it needs. A permission can only be
    /// granted through [`PluginManager::grant_permission`] if it appears here.
    pub permissions: Vec<PluginPermission>,
    /// Current lifecycle state.
    pub state: PluginState,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A capability a plugin can request from the host.
pub enum PluginPermission {
    /// Make network requests.
    NetworkAccess,
    /// Read or write files on the host filesystem.
    FileSystemAccess,
    /// Read or write the system clipboard.
    ClipboardAccess,
    /// Post system notifications.
    Notifications,
    /// Read the device's geographic location.
    Geolocation,
    /// Access a camera device.
    Camera,
    /// Access a microphone device.
    Microphone,
    /// Persist plugin-local data.
    Storage,
    /// Keep running while its host view is not in the foreground.
    BackgroundExecution,
    /// Host-defined permission; the string is the host's own identifier. Because
    /// hosts define the vocabulary, two `Custom` values only compare equal when
    /// the strings match exactly.
    Custom(String),
}
impl PluginPermission {
    /// Returns whether this permission is considered sensitive: filesystem
    /// access, geolocation, camera, microphone, or background execution.
    ///
    /// Network, clipboard, notification, storage and custom permissions are not
    /// covered and are treated as non-sensitive by this classification.
    pub fn is_sensitive(&self) -> bool {
        matches!(
            self,
            PluginPermission::FileSystemAccess
                | PluginPermission::Geolocation
                | PluginPermission::Camera
                | PluginPermission::Microphone
                | PluginPermission::BackgroundExecution
        )
    }
}
/// Behaviour a plugin must implement to be managed by [`PluginManager`].
///
/// Implementors must be safe to share across threads (`Send + Sync`). All
/// methods take `&mut self`, so the manager serialises access to a plugin; there
/// is no internal concurrency requirement beyond `Send + Sync`.
pub trait Plugin: Send + Sync {
    /// Borrows the plugin's metadata, including its current state.
    fn info(&self) -> &PluginInfo;
    /// Mutably borrows the plugin's metadata. The manager uses this to stamp the
    /// assigned id and to update the state, so implementations should not rely
    /// on `id` remaining zero after registration.
    fn info_mut(&mut self) -> &mut PluginInfo;
    /// Called once by [`PluginManager::register`] before the plugin is stored.
    /// Returning `Err` aborts registration, and the plugin is not retained.
    fn on_load(&mut self) -> Result<(), PluginError>;
    /// Called when the plugin is unloaded from the system.
    /// The default implementation is a no-op.
    /// Override to perform cleanup (closing files, releasing resources).
    fn on_unload(&mut self) {}
    /// Called by [`PluginManager::enable`] before the plugin becomes enabled.
    /// Returning `Err` aborts the transition and leaves the previous state intact.
    fn on_enable(&mut self) -> Result<(), PluginError>;
    /// Called when the plugin is disabled.
    /// The default implementation is a no-op.
    /// Override to handle disable logic (saving state, notifying subsystems).
    fn on_disable(&mut self) {}
    /// Handles an incoming message and optionally returns a response.
    ///
    /// [`PluginManager::send_message`] and [`PluginManager::broadcast`] only call
    /// this for enabled plugins, and both treat `None` as "no reply" rather than
    /// as an error.
    fn handle_message(&mut self, message: &str) -> Option<String>;
    /// Returns `self` as [`Any`] for downcasting to the concrete plugin type.
    fn as_any(&self) -> &dyn Any;
    /// Returns `self` as mutable [`Any`] for downcasting to the concrete plugin type.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
#[derive(Debug, Clone)]
/// Error type used throughout the plugin API.
pub struct PluginError {
    /// Human-readable description of the failure.
    pub message: String,
    /// Optional host-defined error code. Also rendered into `Display` output,
    /// but only when present.
    pub code: Option<u32>,
}
impl PluginError {
    /// Creates an error carrying only a message, with no error code.
    pub fn new(message: String) -> Self {
        Self { message, code: None }
    }
    /// Creates an error carrying both a message and a host-defined code.
    pub fn with_code(message: String, code: u32) -> Self {
        Self { message, code: Some(code) }
    }
}
impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(code) = self.code {
            write!(f, "PluginError ({}): {}", code, self.message)
        } else {
            write!(f, "PluginError: {}", self.message)
        }
    }
}
impl std::error::Error for PluginError {}
/// Owns the registered plugins and the permissions granted to them.
///
/// Plugins and their permission lists are both keyed by [`PluginId`]. Granting a
/// permission records it in the allowance list, which is deliberately independent
/// of [`PluginInfo::permissions`]: the latter stays as the plugin declared it, so
/// grants can be revoked back to that baseline. Plugin iteration order (used by
/// `list`, `list_enabled` and `broadcast`) follows the backing hash map and is
/// therefore unspecified.
pub struct PluginManager {
    plugins: HashMap<PluginId, Box<dyn Plugin>>,
    next_id: PluginId,
    allowed_permissions: HashMap<PluginId, Vec<PluginPermission>>,
}
impl PluginManager {
    /// Creates a manager with no plugins and no granted permissions.
    /// The first plugin registered receives id 1.
    pub fn new() -> Self {
        Self { plugins: HashMap::new(), next_id: 1, allowed_permissions: HashMap::new() }
    }
    /// Registers `plugin`, assigning it the next free id and setting its state to
    /// [`PluginState::Installed`].
    ///
    /// `Plugin::on_load` runs before the plugin is stored: if it returns `Err`, the
    /// plugin is not retained (the id is still consumed) and the error is
    /// propagated unchanged.
    pub fn register(&mut self, mut plugin: Box<dyn Plugin>) -> Result<PluginId, PluginError> {
        let id = self.next_id;
        self.next_id += 1;
        plugin.info_mut().id = id;
        plugin.info_mut().state = PluginState::Installed;
        plugin.on_load()?;
        self.plugins.insert(id, plugin);
        Ok(id)
    }
    /// Removes the plugin with `id` and drops its granted permissions.
    ///
    /// An enabled plugin first receives `Plugin::on_disable`, then every plugin,
    /// enabled or not, receives `Plugin::on_unload`. Returns an error whose
    /// message names the missing id when no such plugin is registered.
    pub fn unregister(&mut self, id: PluginId) -> Result<(), PluginError> {
        if let Some(mut plugin) = self.plugins.remove(&id) {
            if plugin.info().state == PluginState::Enabled {
                plugin.on_disable();
            }
            plugin.on_unload();
            self.allowed_permissions.remove(&id);
            Ok(())
        } else {
            Err(PluginError::new(format!("Plugin {id} not found")))
        }
    }
    /// Moves the plugin into [`PluginState::Enabled`], calling `Plugin::on_enable`
    /// first. Only valid from [`PluginState::Installed`] or [`PluginState::Disabled`].
    ///
    /// Propagates any error from `on_enable` without changing the state. An
    /// unknown id, or a plugin in any other state, produces an error instead.
    pub fn enable(&mut self, id: PluginId) -> Result<(), PluginError> {
        if let Some(plugin) = self.plugins.get_mut(&id) {
            if plugin.info().state == PluginState::Disabled
                || plugin.info().state == PluginState::Installed
            {
                plugin.on_enable()?;
                plugin.info_mut().state = PluginState::Enabled;
                Ok(())
            } else {
                Err(PluginError::new(format!(
                    "Cannot enable plugin in state {:?}",
                    plugin.info().state
                )))
            }
        } else {
            Err(PluginError::new(format!("Plugin {id} not found")))
        }
    }
    /// Moves the plugin into [`PluginState::Disabled`], running `Plugin::on_disable`
    /// first. Only valid while the plugin is [`PluginState::Enabled`].
    ///
    /// `on_disable` cannot fail, so the only error cases are an unknown id or a
    /// plugin that is not currently enabled.
    pub fn disable(&mut self, id: PluginId) -> Result<(), PluginError> {
        if let Some(plugin) = self.plugins.get_mut(&id) {
            if plugin.info().state == PluginState::Enabled {
                plugin.on_disable();
                plugin.info_mut().state = PluginState::Disabled;
                Ok(())
            } else {
                Err(PluginError::new(format!(
                    "Cannot disable plugin in state {:?}",
                    plugin.info().state
                )))
            }
        } else {
            Err(PluginError::new(format!("Plugin {id} not found")))
        }
    }
    /// Records that `permission` is allowed for the plugin, but only if the plugin
    /// both exists and declared that permission in [`PluginInfo::permissions`].
    /// Returns `false` in every other case, so a `true` result means the grant took effect.
    ///
    /// Granting the same permission repeatedly appends duplicate entries.
    pub fn grant_permission(&mut self, id: PluginId, permission: PluginPermission) -> bool {
        if let Some(plugin) = self.plugins.get(&id) {
            if plugin.info().permissions.contains(&permission) {
                self.allowed_permissions.entry(id).or_default().push(permission);
                true
            } else {
                false
            }
        } else {
            false
        }
    }
    /// Removes every recorded grant of `permission` for the plugin. Removing a
    /// permission that was never granted, or naming an unknown plugin, is a no-op.
    pub fn revoke_permission(&mut self, id: PluginId, permission: &PluginPermission) {
        if let Some(perms) = self.allowed_permissions.get_mut(&id) {
            perms.retain(|p| p != permission);
        }
    }
    /// Returns whether `permission` has been granted to the plugin. `false` for an
    /// unknown plugin or one with no granted permissions.
    pub fn has_permission(&self, id: PluginId, permission: &PluginPermission) -> bool {
        self.allowed_permissions.get(&id).map(|perms| perms.contains(permission)).unwrap_or(false)
    }
    /// Returns the plugin with `id`, or `None` if it is not registered.
    pub fn get(&self, id: PluginId) -> Option<&dyn Plugin> {
        self.plugins.get(&id).map(|p| p.as_ref())
    }
    /// Runs `f` against the plugin with `id` and returns its result, or `None` if
    /// no such plugin is registered.
    ///
    /// The plugin cannot be accessed after `f` returns, so this is the way to
    /// downcast (`as_any_mut`) and call plugin-specific methods.
    pub fn with_plugin<F, R>(&mut self, id: PluginId, f: F) -> Option<R>
    where
        F: FnOnce(&mut dyn Plugin) -> R,
    {
        self.plugins.get_mut(&id).map(|p| f(p.as_mut()))
    }
    /// Returns the metadata of every registered plugin in unspecified order.
    pub fn list(&self) -> Vec<&PluginInfo> {
        self.plugins.values().map(|p| p.info()).collect()
    }
    /// Returns the metadata of the registered plugins that are currently enabled,
    /// in unspecified order. Empty when nothing is enabled.
    pub fn list_enabled(&self) -> Vec<&PluginInfo> {
        self.plugins
            .values()
            .filter(|p| p.info().state == PluginState::Enabled)
            .map(|p| p.info())
            .collect()
    }
    /// Delivers `message` to one plugin and returns its response.
    ///
    /// `None` means either that no plugin is registered under `id`, or that the
    /// plugin is not [`PluginState::Enabled`], or that it returned `None` itself;
    /// the three cases are not distinguishable from the return value.
    pub fn send_message(&mut self, id: PluginId, message: &str) -> Option<String> {
        if let Some(plugin) = self.plugins.get_mut(&id) {
            if plugin.info().state == PluginState::Enabled {
                plugin.handle_message(message)
            } else {
                None
            }
        } else {
            None
        }
    }
    /// Delivers `message` to every enabled plugin and collects the replies.
    ///
    /// Only plugins that return `Some` contribute an entry, so the result may be
    /// empty even when plugins are enabled, and its order is unspecified.
    pub fn broadcast(&mut self, message: &str) -> Vec<(PluginId, String)> {
        let mut results = Vec::new();
        for (&id, plugin) in &mut self.plugins {
            if plugin.info().state == PluginState::Enabled {
                if let Some(response) = plugin.handle_message(message) {
                    results.push((id, response));
                }
            }
        }
        results
    }
    /// Removes every plugin and drops all granted permissions.
    ///
    /// As in `unregister`, enabled plugins get `Plugin::on_disable` followed by
    /// `Plugin::on_unload`. The id counter is not reset, so ids handed out before
    /// a `clear` are never reused.
    pub fn clear(&mut self) {
        let plugins = core::mem::take(&mut self.plugins);
        for (_, mut plugin) in plugins {
            if plugin.info().state == PluginState::Enabled {
                plugin.on_disable();
            }
            plugin.on_unload();
        }
        self.allowed_permissions.clear();
    }
}
crate::impl_default_via_new!(PluginManager);

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_manager_new() {
        let mgr = PluginManager::new();
        assert!(mgr.list().is_empty());
        assert!(mgr.list_enabled().is_empty());
    }

    #[test]
    fn test_plugin_manager_register() {
        let mut mgr = PluginManager::new();
        let plugin = ContentPlugin::new("test-plugin", "1.0.0");
        let id = mgr.register(Box::new(plugin)).unwrap();
        assert_eq!(id, 1);
        assert_eq!(mgr.list().len(), 1);
        assert_eq!(mgr.list()[0].name, "test-plugin");
        assert_eq!(mgr.list()[0].state, PluginState::Installed);
    }

    #[test]
    fn test_plugin_manager_register_increments_id() {
        let mut mgr = PluginManager::new();
        let id1 = mgr.register(Box::new(ContentPlugin::new("p1", "1.0"))).unwrap();
        let id2 = mgr.register(Box::new(ContentPlugin::new("p2", "1.0"))).unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn test_plugin_manager_unregister() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        assert!(mgr.unregister(id).is_ok());
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_plugin_manager_unregister_nonexistent() {
        let mut mgr = PluginManager::new();
        let result = mgr.unregister(999);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("not found"));
    }

    #[test]
    fn test_plugin_manager_enable() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        assert!(mgr.enable(id).is_ok());
        assert_eq!(mgr.list()[0].state, PluginState::Enabled);
        assert_eq!(mgr.list_enabled().len(), 1);
    }

    #[test]
    fn test_plugin_manager_enable_nonexistent() {
        let mut mgr = PluginManager::new();
        let result = mgr.enable(999);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_manager_disable() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        mgr.enable(id).unwrap();
        assert!(mgr.disable(id).is_ok());
        assert_eq!(mgr.list()[0].state, PluginState::Disabled);
    }

    #[test]
    fn test_plugin_manager_disable_when_not_enabled() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        // Plugin is in Installed state, not Enabled — should fail
        let result = mgr.disable(id);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_manager_plugin_lifecycle() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("cycle", "0.1"))).unwrap();
        assert_eq!(mgr.list()[0].state, PluginState::Installed);
        mgr.enable(id).unwrap();
        assert_eq!(mgr.list()[0].state, PluginState::Enabled);
        mgr.disable(id).unwrap();
        assert_eq!(mgr.list()[0].state, PluginState::Disabled);
        mgr.unregister(id).unwrap();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_plugin_manager_list() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(ContentPlugin::new("a", "1.0"))).unwrap();
        mgr.register(Box::new(ContentPlugin::new("b", "2.0"))).unwrap();
        let list = mgr.list();
        assert_eq!(list.len(), 2);
        let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[test]
    fn test_plugin_manager_list_enabled() {
        let mut mgr = PluginManager::new();
        let id_a = mgr.register(Box::new(ContentPlugin::new("a", "1.0"))).unwrap();
        let _id_b = mgr.register(Box::new(ContentPlugin::new("b", "1.0"))).unwrap();
        mgr.enable(id_a).unwrap();
        let enabled = mgr.list_enabled();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "a");
    }

    #[test]
    fn test_plugin_manager_get() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        let plugin = mgr.get(id);
        assert!(plugin.is_some());
        assert_eq!(plugin.unwrap().info().name, "test");
        assert!(mgr.get(999).is_none());
    }

    #[test]
    fn test_plugin_manager_with_plugin() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        let result = mgr.with_plugin(id, |p| p.info().name.clone());
        assert_eq!(result, Some("test".to_string()));
        let none_result = mgr.with_plugin(999, |p| p.info().name.clone());
        assert!(none_result.is_none());
    }

    #[test]
    fn test_plugin_manager_send_message_disabled_returns_none() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        // Plugin is Installed (not Enabled), so send_message returns None
        assert!(mgr.send_message(id, "hello").is_none());
    }

    #[test]
    fn test_plugin_manager_send_message_nonexistent() {
        let mut mgr = PluginManager::new();
        assert!(mgr.send_message(999, "hello").is_none());
    }

    #[test]
    fn test_plugin_manager_broadcast_empty() {
        let mut mgr = PluginManager::new();
        let results = mgr.broadcast("hello");
        assert!(results.is_empty());
    }

    #[test]
    fn test_plugin_manager_clear() {
        let mut mgr = PluginManager::new();
        mgr.register(Box::new(ContentPlugin::new("a", "1.0"))).unwrap();
        mgr.register(Box::new(ContentPlugin::new("b", "1.0"))).unwrap();
        assert_eq!(mgr.list().len(), 2);
        mgr.clear();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_plugin_manager_grant_permission() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        // ContentPlugin has NetworkAccess permission
        assert!(mgr.grant_permission(id, PluginPermission::NetworkAccess));
        assert!(mgr.has_permission(id, &PluginPermission::NetworkAccess));
    }

    #[test]
    fn test_plugin_manager_grant_permission_not_requested() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        // ContentPlugin does NOT have Camera permission in its list
        assert!(!mgr.grant_permission(id, PluginPermission::Camera));
    }

    #[test]
    fn test_plugin_manager_revoke_permission() {
        let mut mgr = PluginManager::new();
        let id = mgr.register(Box::new(ContentPlugin::new("test", "1.0"))).unwrap();
        mgr.grant_permission(id, PluginPermission::NetworkAccess);
        assert!(mgr.has_permission(id, &PluginPermission::NetworkAccess));
        mgr.revoke_permission(id, &PluginPermission::NetworkAccess);
        assert!(!mgr.has_permission(id, &PluginPermission::NetworkAccess));
    }

    #[test]
    fn test_plugin_permission_is_sensitive() {
        assert!(PluginPermission::FileSystemAccess.is_sensitive());
        assert!(PluginPermission::Geolocation.is_sensitive());
        assert!(PluginPermission::Camera.is_sensitive());
        assert!(PluginPermission::Microphone.is_sensitive());
        assert!(PluginPermission::BackgroundExecution.is_sensitive());
        assert!(!PluginPermission::NetworkAccess.is_sensitive());
        assert!(!PluginPermission::Notifications.is_sensitive());
        assert!(!PluginPermission::ClipboardAccess.is_sensitive());
    }

    #[test]
    fn test_plugin_error_new() {
        let err = PluginError::new("something failed".to_string());
        assert_eq!(err.message, "something failed");
        assert!(err.code.is_none());
    }

    #[test]
    fn test_plugin_error_with_code() {
        let err = PluginError::with_code("error code 42".to_string(), 42);
        assert_eq!(err.message, "error code 42");
        assert_eq!(err.code, Some(42));
    }

    #[test]
    fn test_plugin_error_display() {
        let err = PluginError::new("test error".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("test error"));

        let err2 = PluginError::with_code("code error".to_string(), 7);
        let msg2 = format!("{}", err2);
        assert!(msg2.contains("7"));
    }

    #[test]
    fn test_content_plugin_new() {
        let plugin = ContentPlugin::new("my-plugin", "0.1.0");
        assert_eq!(plugin.info.name, "my-plugin");
        assert_eq!(plugin.info.version, "0.1.0");
        assert_eq!(plugin.info.state, PluginState::NotInstalled);
        assert!(plugin.info.permissions.contains(&PluginPermission::NetworkAccess));
    }

    #[test]
    fn test_content_plugin_register_handler_and_process() {
        let mut plugin = ContentPlugin::new("handler-plugin", "1.0");
        plugin.register_handler("text/plain", |content| Some(format!("processed: {}", content)));
        let result = plugin.process("text/plain", "hello");
        assert_eq!(result, Some("processed: hello".to_string()));
    }

    #[test]
    fn test_content_plugin_process_unregistered_type() {
        let plugin = ContentPlugin::new("test", "1.0");
        let result = plugin.process("text/plain", "hello");
        assert!(result.is_none());
    }

    #[test]
    fn test_content_plugin_on_load_and_on_enable() {
        let mut plugin = ContentPlugin::new("test", "1.0");
        assert!(plugin.on_load().is_ok());
        assert!(plugin.on_enable().is_ok());
    }

    #[test]
    fn test_content_plugin_as_any() {
        let plugin = ContentPlugin::new("test", "1.0");
        let any: &dyn Any = plugin.as_any();
        assert!(any.is::<ContentPlugin>());
    }

    #[test]
    fn test_plugin_state_discriminants() {
        assert_ne!(PluginState::NotInstalled as u8, PluginState::Installed as u8);
        assert_ne!(PluginState::Enabled as u8, PluginState::Disabled as u8);
        assert_ne!(PluginState::Blocked as u8, PluginState::Error as u8);
    }

    #[test]
    fn test_plugin_manager_default() {
        let mgr = PluginManager::default();
        assert!(mgr.list().is_empty());
    }
}
/// A concrete [`Plugin`] that transforms content by MIME type.
///
/// Handlers are keyed by content type string, so registering a handler for a type
/// that already has one replaces it.
pub struct ContentPlugin {
    info: PluginInfo,
    content_handlers: HashMap<String, ContentHandler>,
}
impl ContentPlugin {
    /// Creates a plugin named `name` at version `version`, with empty description
    /// and author, no homepage, state [`PluginState::NotInstalled`] and id 0.
    ///
    /// The declared permissions are exactly `[PluginPermission::NetworkAccess]`;
    /// no other permission can be granted until the set is extended via
    /// `as_any_mut`.
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            info: PluginInfo {
                id: 0,
                name: name.to_string(),
                version: version.to_string(),
                description: String::new(),
                author: String::new(),
                homepage: None,
                permissions: vec![PluginPermission::NetworkAccess],
                state: PluginState::NotInstalled,
            },
            content_handlers: HashMap::new(),
        }
    }
    /// Registers `handler` for `content_type`, replacing any handler already
    /// registered for that content type.
    pub fn register_handler<F>(&mut self, content_type: &str, handler: F)
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        self.content_handlers.insert(content_type.to_string(), Box::new(handler));
    }
    /// Runs the handler registered for `content_type` against `content`, returning
    /// whatever it produces. `None` when no handler is registered for that content
    /// type, or when the handler itself returns `None`.
    pub fn process(&self, content_type: &str, content: &str) -> Option<String> {
        self.content_handlers.get(content_type).and_then(|handler| handler(content))
    }
}
impl Plugin for ContentPlugin {
    fn info(&self) -> &PluginInfo {
        &self.info
    }
    fn info_mut(&mut self) -> &mut PluginInfo {
        &mut self.info
    }
    fn on_load(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
    fn on_enable(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
    fn handle_message(&mut self, _message: &str) -> Option<String> {
        None
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
