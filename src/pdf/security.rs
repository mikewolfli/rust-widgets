use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    None,
    RC4_40,
    RC4_128,
    Aes128,
    Aes256,
}

impl Default for EncryptionAlgorithm {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    Print,
    Modify,
    Copy,
    Annotate,
    FillForms,
    Extract,
    Assemble,
    PrintHighQuality,
}

#[derive(Debug, Clone)]
pub struct SecuritySettings {
    pub user_password: String,
    pub owner_password: String,
    pub encryption_algorithm: EncryptionAlgorithm,
    pub key_length: u32,
    pub permissions: HashSet<Permission>,
    pub encrypt_metadata: bool,
}

impl SecuritySettings {
    pub fn new() -> Self {
        let mut permissions = HashSet::new();
        permissions.insert(Permission::Print);
        permissions.insert(Permission::Copy);
        permissions.insert(Permission::Annotate);
        permissions.insert(Permission::FillForms);
        
        Self {
            user_password: String::new(),
            owner_password: String::new(),
            encryption_algorithm: EncryptionAlgorithm::None,
            key_length: 128,
            permissions,
            encrypt_metadata: true,
        }
    }

    pub fn with_user_password(mut self, password: String) -> Self {
        self.user_password = password;
        self
    }

    pub fn with_owner_password(mut self, password: String) -> Self {
        self.owner_password = password;
        self
    }

    pub fn with_encryption(mut self, algorithm: EncryptionAlgorithm) -> Self {
        self.encryption_algorithm = algorithm;
        self.key_length = match algorithm {
            EncryptionAlgorithm::RC4_40 => 40,
            EncryptionAlgorithm::RC4_128 => 128,
            EncryptionAlgorithm::Aes128 => 128,
            EncryptionAlgorithm::Aes256 => 256,
            EncryptionAlgorithm::None => 0,
        };
        self
    }

    pub fn with_permissions(mut self, permissions: HashSet<Permission>) -> Self {
        self.permissions = permissions;
        self
    }

    pub fn grant_permission(&mut self, permission: Permission) {
        self.permissions.insert(permission);
    }

    pub fn revoke_permission(&mut self, permission: Permission) {
        self.permissions.remove(&permission);
    }

    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    pub fn is_encrypted(&self) -> bool {
        self.encryption_algorithm != EncryptionAlgorithm::None
    }

    pub fn requires_password(&self) -> bool {
        !self.user_password.is_empty() || !self.owner_password.is_empty()
    }
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct DigitalSignature {
    pub name: String,
    pub location: String,
    pub reason: String,
    pub contact_info: String,
    pub signing_time: String,
    pub signature_data: Vec<u8>,
    pub cert_data: Vec<u8>,
    pub is_valid: bool,
    pub validation_message: String,
}

impl DigitalSignature {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            location: String::new(),
            reason: String::new(),
            contact_info: String::new(),
            signing_time: String::new(),
            signature_data: Vec::new(),
            cert_data: Vec::new(),
            is_valid: false,
            validation_message: String::new(),
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn with_location(mut self, location: String) -> Self {
        self.location = location;
        self
    }

    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = reason;
        self
    }

    pub fn with_contact(mut self, contact: String) -> Self {
        self.contact_info = contact;
        self
    }

    pub fn with_signature_data(mut self, data: Vec<u8>) -> Self {
        self.signature_data = data;
        self
    }

    pub fn with_cert_data(mut self, data: Vec<u8>) -> Self {
        self.cert_data = data;
        self
    }
}

impl Default for DigitalSignature {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SecurityManager {
    settings: SecuritySettings,
    signatures: Vec<DigitalSignature>,
    is_authenticated: bool,
    authentication_error: Option<String>,
}

impl SecurityManager {
    pub fn new() -> Self {
        Self {
            settings: SecuritySettings::new(),
            signatures: Vec::new(),
            is_authenticated: false,
            authentication_error: None,
        }
    }

    pub fn with_settings(settings: SecuritySettings) -> Self {
        Self {
            settings,
            signatures: Vec::new(),
            is_authenticated: false,
            authentication_error: None,
        }
    }

    pub fn authenticate(&mut self, password: &str) -> bool {
        if !self.settings.requires_password() {
            self.is_authenticated = true;
            return true;
        }

        if password == self.settings.user_password || password == self.settings.owner_password {
            self.is_authenticated = true;
            self.authentication_error = None;
            true
        } else {
            self.is_authenticated = false;
            self.authentication_error = Some("Invalid password".to_string());
            false
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.is_authenticated || !self.settings.requires_password()
    }

    pub fn can_print(&self) -> bool {
        self.is_authenticated() && self.settings.has_permission(&Permission::Print)
    }

    pub fn can_modify(&self) -> bool {
        self.is_authenticated() && self.settings.has_permission(&Permission::Modify)
    }

    pub fn can_copy(&self) -> bool {
        self.is_authenticated() && self.settings.has_permission(&Permission::Copy)
    }

    pub fn can_annotate(&self) -> bool {
        self.is_authenticated() && self.settings.has_permission(&Permission::Annotate)
    }

    pub fn can_fill_forms(&self) -> bool {
        self.is_authenticated() && self.settings.has_permission(&Permission::FillForms)
    }

    pub fn can_extract(&self) -> bool {
        self.is_authenticated() && self.settings.has_permission(&Permission::Extract)
    }

    pub fn can_assemble(&self) -> bool {
        self.is_authenticated() && self.settings.has_permission(&Permission::Assemble)
    }

    pub fn can_print_high_quality(&self) -> bool {
        self.is_authenticated() && self.settings.has_permission(&Permission::PrintHighQuality)
    }

    pub fn add_signature(&mut self, signature: DigitalSignature) {
        self.signatures.push(signature);
    }

    pub fn get_signatures(&self) -> &[DigitalSignature] {
        &self.signatures
    }

    pub fn has_valid_signatures(&self) -> bool {
        !self.signatures.is_empty() && self.signatures.iter().all(|s| s.is_valid)
    }

    pub fn get_settings(&self) -> &SecuritySettings {
        &self.settings
    }

    pub fn set_settings(&mut self, settings: SecuritySettings) {
        self.settings = settings;
        self.is_authenticated = false;
    }

    pub fn get_authentication_error(&self) -> Option<&String> {
        self.authentication_error.as_ref()
    }

    pub fn clear_authentication(&mut self) {
        self.is_authenticated = false;
        self.authentication_error = None;
    }

    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    pub fn valid_signature_count(&self) -> usize {
        self.signatures.iter().filter(|s| s.is_valid).count()
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_settings() {
        let mut settings = SecuritySettings::new()
            .with_user_password("user123".to_string())
            .with_owner_password("owner456".to_string())
            .with_encryption(EncryptionAlgorithm::Aes256);

        assert!(settings.requires_password());
        assert!(settings.is_encrypted());
        assert_eq!(settings.key_length, 256);

        settings.revoke_permission(Permission::Print);
        assert!(!settings.has_permission(&Permission::Print));

        settings.grant_permission(Permission::Print);
        assert!(settings.has_permission(&Permission::Print));
    }

    #[test]
    fn test_security_manager_authentication() {
        let settings = SecuritySettings::new()
            .with_user_password("testpass".to_string());

        let mut manager = SecurityManager::with_settings(settings);

        assert!(!manager.is_authenticated());
        assert!(!manager.authenticate("wrongpass"));
        assert!(!manager.is_authenticated());

        assert!(manager.authenticate("testpass"));
        assert!(manager.is_authenticated());
    }

    #[test]
    fn test_security_manager_permissions() {
        let mut settings = SecuritySettings::new();
        settings.revoke_permission(Permission::Print);
        settings.revoke_permission(Permission::Copy);

        let mut manager = SecurityManager::with_settings(settings);
        manager.authenticate("");

        assert!(!manager.can_print());
        assert!(!manager.can_copy());
        assert!(manager.can_annotate());
    }

    #[test]
    fn test_digital_signature() {
        let signature = DigitalSignature::new()
            .with_name("John Doe".to_string())
            .with_location("New York".to_string())
            .with_reason("Document approval".to_string());

        assert_eq!(signature.name, "John Doe");
        assert_eq!(signature.location, "New York");
        assert_eq!(signature.reason, "Document approval");
    }

    #[test]
    fn test_signature_management() {
        let mut manager = SecurityManager::new();

        let signature = DigitalSignature::new()
            .with_name("Test".to_string());

        manager.add_signature(signature);

        assert_eq!(manager.signature_count(), 1);
        assert_eq!(manager.valid_signature_count(), 0);
        assert!(!manager.has_valid_signatures());
    }
}
