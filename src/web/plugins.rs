use std::any::Any;
use std::collections::HashMap;
pub type PluginId = u64;
/// Handler type for content transformation plugins.
pub type ContentHandler = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginState {
    NotInstalled,
    Installed,
    Enabled,
    Disabled,
    Blocked,
    Error,
}
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub homepage: Option<String>,
    pub permissions: Vec<PluginPermission>,
    pub state: PluginState,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PluginPermission {
    NetworkAccess,
    FileSystemAccess,
    ClipboardAccess,
    Notifications,
    Geolocation,
    Camera,
    Microphone,
    Storage,
    BackgroundExecution,
    Custom(String),
}
impl PluginPermission {
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
pub trait Plugin: Send + Sync {
    fn info(&self) -> &PluginInfo;
    fn info_mut(&mut self) -> &mut PluginInfo;
    fn on_load(&mut self) -> Result<(), PluginError>;
    fn on_unload(&mut self) {}
    fn on_enable(&mut self) -> Result<(), PluginError>;
    fn on_disable(&mut self) {}
    fn handle_message(&mut self, message: &str) -> Option<String>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
#[derive(Debug, Clone)]
pub struct PluginError {
    pub message: String,
    pub code: Option<u32>,
}
impl PluginError {
    pub fn new(message: String) -> Self {
        Self {
            message,
            code: None,
        }
    }
    pub fn with_code(message: String, code: u32) -> Self {
        Self {
            message,
            code: Some(code),
        }
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
pub struct PluginManager {
    plugins: HashMap<PluginId, Box<dyn Plugin>>,
    next_id: PluginId,
    allowed_permissions: HashMap<PluginId, Vec<PluginPermission>>,
}
impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            next_id: 1,
            allowed_permissions: HashMap::new(),
        }
    }
    pub fn register(&mut self, mut plugin: Box<dyn Plugin>) -> Result<PluginId, PluginError> {
        let id = self.next_id;
        self.next_id += 1;
        plugin.info_mut().id = id;
        plugin.info_mut().state = PluginState::Installed;
        plugin.on_load()?;
        self.plugins.insert(id, plugin);
        Ok(id)
    }
    pub fn unregister(&mut self, id: PluginId) -> Result<(), PluginError> {
        if let Some(mut plugin) = self.plugins.remove(&id) {
            if plugin.info().state == PluginState::Enabled {
                plugin.on_disable();
            }
            plugin.on_unload();
            self.allowed_permissions.remove(&id);
            Ok(())
        } else {
            Err(PluginError::new(format!("Plugin {} not found", id)))
        }
    }
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
            Err(PluginError::new(format!("Plugin {} not found", id)))
        }
    }
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
            Err(PluginError::new(format!("Plugin {} not found", id)))
        }
    }
    pub fn grant_permission(&mut self, id: PluginId, permission: PluginPermission) -> bool {
        if let Some(plugin) = self.plugins.get(&id) {
            if plugin.info().permissions.contains(&permission) {
                self.allowed_permissions
                    .entry(id)
                    .or_default()
                    .push(permission);
                true
            } else {
                false
            }
        } else {
            false
        }
    }
    pub fn revoke_permission(&mut self, id: PluginId, permission: &PluginPermission) {
        if let Some(perms) = self.allowed_permissions.get_mut(&id) {
            perms.retain(|p| p != permission);
        }
    }
    pub fn has_permission(&self, id: PluginId, permission: &PluginPermission) -> bool {
        self.allowed_permissions
            .get(&id)
            .map(|perms| perms.contains(permission))
            .unwrap_or(false)
    }
    pub fn get(&self, id: PluginId) -> Option<&dyn Plugin> {
        self.plugins.get(&id).map(|p| p.as_ref())
    }
    pub fn with_plugin<F, R>(&mut self, id: PluginId, f: F) -> Option<R>
    where
        F: FnOnce(&mut dyn Plugin) -> R,
    {
        self.plugins.get_mut(&id).map(|p| f(p.as_mut()))
    }
    pub fn list(&self) -> Vec<&PluginInfo> {
        self.plugins.values().map(|p| p.info()).collect()
    }
    pub fn list_enabled(&self) -> Vec<&PluginInfo> {
        self.plugins
            .values()
            .filter(|p| p.info().state == PluginState::Enabled)
            .map(|p| p.info())
            .collect()
    }
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
    pub fn clear(&mut self) {
        for (_, mut plugin) in self.plugins.drain() {
            if plugin.info().state == PluginState::Enabled {
                plugin.on_disable();
            }
            plugin.on_unload();
        }
        self.allowed_permissions.clear();
    }
}
impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

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
        let id1 = mgr
            .register(Box::new(ContentPlugin::new("p1", "1.0")))
            .unwrap();
        let id2 = mgr
            .register(Box::new(ContentPlugin::new("p2", "1.0")))
            .unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn test_plugin_manager_unregister() {
        let mut mgr = PluginManager::new();
        let id = mgr
            .register(Box::new(ContentPlugin::new("test", "1.0")))
            .unwrap();
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
        let id = mgr
            .register(Box::new(ContentPlugin::new("test", "1.0")))
            .unwrap();
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
        let id = mgr
            .register(Box::new(ContentPlugin::new("test", "1.0")))
            .unwrap();
        mgr.enable(id).unwrap();
        assert!(mgr.disable(id).is_ok());
        assert_eq!(mgr.list()[0].state, PluginState::Disabled);
    }

    #[test]
    fn test_plugin_manager_disable_when_not_enabled() {
        let mut mgr = PluginManager::new();
        let id = mgr
            .register(Box::new(ContentPlugin::new("test", "1.0")))
            .unwrap();
        // Plugin is in Installed state, not Enabled — should fail
        let result = mgr.disable(id);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_manager_plugin_lifecycle() {
        let mut mgr = PluginManager::new();
        let id = mgr
            .register(Box::new(ContentPlugin::new("cycle", "0.1")))
            .unwrap();
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
        mgr.register(Box::new(ContentPlugin::new("a", "1.0")))
            .unwrap();
        mgr.register(Box::new(ContentPlugin::new("b", "2.0")))
            .unwrap();
        let list = mgr.list();
        assert_eq!(list.len(), 2);
        let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[test]
    fn test_plugin_manager_list_enabled() {
        let mut mgr = PluginManager::new();
        let id_a = mgr
            .register(Box::new(ContentPlugin::new("a", "1.0")))
            .unwrap();
        let _id_b = mgr
            .register(Box::new(ContentPlugin::new("b", "1.0")))
            .unwrap();
        mgr.enable(id_a).unwrap();
        let enabled = mgr.list_enabled();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "a");
    }

    #[test]
    fn test_plugin_manager_get() {
        let mut mgr = PluginManager::new();
        let id = mgr
            .register(Box::new(ContentPlugin::new("test", "1.0")))
            .unwrap();
        let plugin = mgr.get(id);
        assert!(plugin.is_some());
        assert_eq!(plugin.unwrap().info().name, "test");
        assert!(mgr.get(999).is_none());
    }

    #[test]
    fn test_plugin_manager_with_plugin() {
        let mut mgr = PluginManager::new();
        let id = mgr
            .register(Box::new(ContentPlugin::new("test", "1.0")))
            .unwrap();
        let result = mgr.with_plugin(id, |p| p.info().name.clone());
        assert_eq!(result, Some("test".to_string()));
        let none_result = mgr.with_plugin(999, |p| p.info().name.clone());
        assert!(none_result.is_none());
    }

    #[test]
    fn test_plugin_manager_send_message_disabled_returns_none() {
        let mut mgr = PluginManager::new();
        let id = mgr
            .register(Box::new(ContentPlugin::new("test", "1.0")))
            .unwrap();
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
        mgr.register(Box::new(ContentPlugin::new("a", "1.0")))
            .unwrap();
        mgr.register(Box::new(ContentPlugin::new("b", "1.0")))
            .unwrap();
        assert_eq!(mgr.list().len(), 2);
        mgr.clear();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_plugin_manager_grant_permission() {
        let mut mgr = PluginManager::new();
        let id = mgr
            .register(Box::new(ContentPlugin::new("test", "1.0")))
            .unwrap();
        // ContentPlugin has NetworkAccess permission
        assert!(mgr.grant_permission(id, PluginPermission::NetworkAccess));
        assert!(mgr.has_permission(id, &PluginPermission::NetworkAccess));
    }

    #[test]
    fn test_plugin_manager_grant_permission_not_requested() {
        let mut mgr = PluginManager::new();
        let id = mgr
            .register(Box::new(ContentPlugin::new("test", "1.0")))
            .unwrap();
        // ContentPlugin does NOT have Camera permission in its list
        assert!(!mgr.grant_permission(id, PluginPermission::Camera));
    }

    #[test]
    fn test_plugin_manager_revoke_permission() {
        let mut mgr = PluginManager::new();
        let id = mgr
            .register(Box::new(ContentPlugin::new("test", "1.0")))
            .unwrap();
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
        assert!(plugin
            .info
            .permissions
            .contains(&PluginPermission::NetworkAccess));
    }

    #[test]
    fn test_content_plugin_register_handler_and_process() {
        let mut plugin = ContentPlugin::new("handler-plugin", "1.0");
        plugin.register_handler("text/plain", |content| {
            Some(format!("processed: {}", content))
        });
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
        assert_ne!(
            PluginState::NotInstalled as u8,
            PluginState::Installed as u8
        );
        assert_ne!(PluginState::Enabled as u8, PluginState::Disabled as u8);
        assert_ne!(PluginState::Blocked as u8, PluginState::Error as u8);
    }

    #[test]
    fn test_plugin_manager_default() {
        let mgr = PluginManager::default();
        assert!(mgr.list().is_empty());
    }
}
pub struct ContentPlugin {
    info: PluginInfo,
    content_handlers: HashMap<String, ContentHandler>,
}
impl ContentPlugin {
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
    pub fn register_handler<F>(&mut self, content_type: &str, handler: F)
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        self.content_handlers
            .insert(content_type.to_string(), Box::new(handler));
    }
    pub fn process(&self, content_type: &str, content: &str) -> Option<String> {
        self.content_handlers
            .get(content_type)
            .and_then(|handler| handler(content))
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
