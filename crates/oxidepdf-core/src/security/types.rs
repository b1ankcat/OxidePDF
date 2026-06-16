use crate::OxideError;
use lopdf::Permissions;
use serde::{Deserialize, Serialize};
use std::fmt;

/// PDF password, encryption, and permission operations.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(try_from = "PdfSecurityOptionsDef", into = "PdfSecurityOptionsDef")]
pub enum PdfSecurityOptions {
    /// Encrypt a PDF with Standard Security Handler passwords.
    Encrypt(SecurityEncryptOptions),
    /// Decrypt a password-protected PDF.
    Decrypt(SecurityDecryptOptions),
    /// Inspect password and permission metadata.
    PermissionsGet(SecurityPermissionGetOptions),
    /// Replace a document's permission policy.
    PermissionsSet(SecurityPermissionSetOptions),
}

impl fmt::Debug for PdfSecurityOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encrypt(options) => formatter.debug_tuple("Encrypt").field(options).finish(),
            Self::Decrypt(options) => formatter.debug_tuple("Decrypt").field(options).finish(),
            Self::PermissionsGet(options) => formatter
                .debug_tuple("PermissionsGet")
                .field(options)
                .finish(),
            Self::PermissionsSet(options) => formatter
                .debug_tuple("PermissionsSet")
                .field(options)
                .finish(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct PdfSecurityOptionsDef {
    encrypt: Option<SecurityEncryptOptions>,
    decrypt: Option<SecurityDecryptOptions>,
    permissions_get: Option<SecurityPermissionGetOptions>,
    permissions_set: Option<SecurityPermissionSetOptions>,
}

impl TryFrom<PdfSecurityOptionsDef> for PdfSecurityOptions {
    type Error = OxideError;

    fn try_from(value: PdfSecurityOptionsDef) -> Result<Self, Self::Error> {
        let operation_count = [
            value.encrypt.is_some(),
            value.decrypt.is_some(),
            value.permissions_get.is_some(),
            value.permissions_set.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();

        if operation_count != 1 {
            return Err(OxideError::InvalidWorkflow {
                reason: "pdf_security must contain exactly one operation".to_owned(),
            });
        }

        if let Some(options) = value.encrypt {
            return Ok(Self::Encrypt(options));
        }
        if let Some(options) = value.decrypt {
            return Ok(Self::Decrypt(options));
        }
        if let Some(options) = value.permissions_get {
            return Ok(Self::PermissionsGet(options));
        }
        if let Some(options) = value.permissions_set {
            return Ok(Self::PermissionsSet(options));
        }

        unreachable!("operation count was already checked");
    }
}

impl From<PdfSecurityOptions> for PdfSecurityOptionsDef {
    fn from(value: PdfSecurityOptions) -> Self {
        match value {
            PdfSecurityOptions::Encrypt(options) => Self {
                encrypt: Some(options),
                ..Self::default()
            },
            PdfSecurityOptions::Decrypt(options) => Self {
                decrypt: Some(options),
                ..Self::default()
            },
            PdfSecurityOptions::PermissionsGet(options) => Self {
                permissions_get: Some(options),
                ..Self::default()
            },
            PdfSecurityOptions::PermissionsSet(options) => Self {
                permissions_set: Some(options),
                ..Self::default()
            },
        }
    }
}

/// Supported encryption algorithms for newly written PDFs.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EncryptionAlgorithm {
    /// AES-256 Standard Security Handler, revision 6.
    #[default]
    Aes256,
    /// AES-128 Standard Security Handler, revision 4.
    Aes128,
    /// Legacy RC4 Standard Security Handler. Explicitly unsupported until fully tested.
    Rc4,
}

/// Explicit document permission policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PermissionPolicy {
    pub print: bool,
    pub modify: bool,
    pub copy: bool,
    pub annotate: bool,
    pub fill_forms: bool,
    pub accessibility: bool,
    pub assemble: bool,
    pub high_quality_print: bool,
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        Self {
            print: true,
            modify: true,
            copy: true,
            annotate: true,
            fill_forms: true,
            accessibility: true,
            assemble: true,
            high_quality_print: true,
        }
    }
}

impl PermissionPolicy {
    pub(super) fn to_lopdf_permissions(&self) -> Permissions {
        let mut permissions = Permissions::empty();
        permissions.set(Permissions::PRINTABLE, self.print);
        permissions.set(Permissions::MODIFIABLE, self.modify);
        permissions.set(Permissions::COPYABLE, self.copy);
        permissions.set(Permissions::ANNOTABLE, self.annotate);
        permissions.set(Permissions::FILLABLE, self.fill_forms);
        permissions.set(Permissions::COPYABLE_FOR_ACCESSIBILITY, self.accessibility);
        permissions.set(Permissions::ASSEMBLABLE, self.assemble);
        permissions.set(
            Permissions::PRINTABLE_IN_HIGH_QUALITY,
            self.high_quality_print,
        );
        permissions
    }

    pub(super) fn from_bits(bits: i64) -> Self {
        let permissions = Permissions::from_bits_retain(bits as u64);
        Self {
            print: permissions.contains(Permissions::PRINTABLE),
            modify: permissions.contains(Permissions::MODIFIABLE),
            copy: permissions.contains(Permissions::COPYABLE),
            annotate: permissions.contains(Permissions::ANNOTABLE),
            fill_forms: permissions.contains(Permissions::FILLABLE),
            accessibility: permissions.contains(Permissions::COPYABLE_FOR_ACCESSIBILITY),
            assemble: permissions.contains(Permissions::ASSEMBLABLE),
            high_quality_print: permissions.contains(Permissions::PRINTABLE_IN_HIGH_QUALITY),
        }
    }
}

/// Options for encrypting a PDF.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SecurityEncryptOptions {
    pub owner_password: String,
    pub user_password: String,
    pub algorithm: EncryptionAlgorithm,
    pub permissions: PermissionPolicy,
}

impl Default for SecurityEncryptOptions {
    fn default() -> Self {
        Self {
            owner_password: String::new(),
            user_password: String::new(),
            algorithm: EncryptionAlgorithm::Aes256,
            permissions: PermissionPolicy::default(),
        }
    }
}

impl fmt::Debug for SecurityEncryptOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecurityEncryptOptions")
            .field("owner_password", &"<redacted>")
            .field("user_password", &"<redacted>")
            .field("algorithm", &self.algorithm)
            .field("permissions", &self.permissions)
            .finish()
    }
}

/// Options for decrypting a PDF.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SecurityDecryptOptions {
    pub password: Option<String>,
}

impl fmt::Debug for SecurityDecryptOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecurityDecryptOptions")
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// Options for inspecting permissions.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SecurityPermissionGetOptions {
    pub password: Option<String>,
}

impl fmt::Debug for SecurityPermissionGetOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecurityPermissionGetOptions")
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// Options for replacing permissions.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SecurityPermissionSetOptions {
    pub owner_password: String,
    pub user_password: String,
    pub algorithm: EncryptionAlgorithm,
    pub permissions: PermissionPolicy,
}

impl Default for SecurityPermissionSetOptions {
    fn default() -> Self {
        Self {
            owner_password: String::new(),
            user_password: String::new(),
            algorithm: EncryptionAlgorithm::Aes256,
            permissions: PermissionPolicy::default(),
        }
    }
}

impl fmt::Debug for SecurityPermissionSetOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecurityPermissionSetOptions")
            .field("owner_password", &"<redacted>")
            .field("user_password", &"<redacted>")
            .field("algorithm", &self.algorithm)
            .field("permissions", &self.permissions)
            .finish()
    }
}

/// JSON permission report emitted by `permissions get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PermissionReport {
    pub encrypted: bool,
    pub handler: Option<String>,
    pub version: Option<i64>,
    pub revision: Option<i64>,
    pub key_length_bits: Option<i64>,
    pub permissions_bits: Option<i64>,
    pub permissions: PermissionPolicy,
}
