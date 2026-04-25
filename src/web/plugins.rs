use std::any::Any;
use std::collections::HashMap;
pub type PluginId = u64;
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
                    .or_insert_with(Vec::new)
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
pub struct ContentPlugin {
    info: PluginInfo,
    content_handlers: HashMap<String, Box<dyn Fn(&str) -> Option<String> + Send + Sync>>,
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
