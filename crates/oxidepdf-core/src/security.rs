use crate::{
    enforce_input_bytes, enforce_max_pages, enforce_output_bytes, ensure_pdf_magic, OxideError,
    PdfArtifact, ResourceLimits, TextArtifact,
};
use lopdf::encryption::crypt_filters::{Aes128CryptFilter, Aes256CryptFilter, CryptFilter};
use lopdf::xref::XrefEntry;
use lopdf::{Document, EncryptionState, EncryptionVersion, Object, Permissions};
use rand::Rng as _;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use zeroize::Zeroizing;

/// PDF password, encryption, and permission operations.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    fn to_lopdf_permissions(&self) -> Permissions {
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

    fn from_bits(bits: i64) -> Self {
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
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionReport {
    pub encrypted: bool,
    pub handler: Option<String>,
    pub version: Option<i64>,
    pub revision: Option<i64>,
    pub key_length_bits: Option<i64>,
    pub permissions_bits: Option<i64>,
    pub permissions: PermissionPolicy,
}

pub fn encrypt_pdf(
    input: &[u8],
    options: &SecurityEncryptOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    ensure_supported_algorithm(options.algorithm)?;
    ensure_explicit_passwords(&options.owner_password, &options.user_password)?;

    let mut document = Document::load_mem(input).map_err(|_| OxideError::ParsePdf)?;
    if document.is_encrypted() {
        return Err(OxideError::EncryptedPdf);
    }
    enforce_max_pages(document.get_pages().len(), limits)?;

    document = normalize_plain_document(document)?;
    apply_encryption(
        &mut document,
        options.algorithm,
        &options.owner_password,
        &options.user_password,
        &options.permissions,
    )?;
    save_security_pdf(document, limits)
}

pub fn decrypt_pdf(
    input: &[u8],
    options: &SecurityDecryptOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    let mut document = Document::load_mem(input).map_err(|_| OxideError::ParsePdf)?;
    if !document.is_encrypted() {
        return Err(OxideError::InvalidInput {
            reason: "PDF is not encrypted".to_owned(),
        });
    }
    ensure_supported_encryption_revision(&document)?;

    let password = required_password(options.password.as_deref())?;
    document = load_decrypted_document(input, &document, &password)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    save_security_pdf(document, limits)
}

pub fn inspect_pdf_permissions(
    input: &[u8],
    options: &SecurityPermissionGetOptions,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    let document = Document::load_mem(input).map_err(|_| OxideError::ParsePdf)?;
    let report = if document.is_encrypted() {
        ensure_supported_encryption_revision(&document)?;
        let password = required_password(options.password.as_deref())?;
        document
            .authenticate_password(&password)
            .map_err(map_lopdf_security_error)?;
        report_from_encrypted_document(&document)?
    } else {
        PermissionReport {
            encrypted: false,
            handler: None,
            version: None,
            revision: None,
            key_length_bits: None,
            permissions_bits: None,
            permissions: PermissionPolicy::default(),
        }
    };

    let text = serde_json::to_string_pretty(&report).map_err(|_| OxideError::Internal)?;
    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

pub fn set_pdf_permissions(
    input: &[u8],
    options: &SecurityPermissionSetOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    ensure_supported_algorithm(options.algorithm)?;
    ensure_explicit_passwords(&options.owner_password, &options.user_password)?;

    let mut document = Document::load_mem(input).map_err(|_| OxideError::ParsePdf)?;
    if document.is_encrypted() {
        ensure_supported_encryption_revision(&document)?;
        let owner_password = Zeroizing::new(options.owner_password.clone());
        document
            .authenticate_owner_password(&owner_password)
            .map_err(map_lopdf_security_error)?;
        document = load_decrypted_document(input, &document, &owner_password)?;
    }
    enforce_max_pages(document.get_pages().len(), limits)?;

    document = normalize_plain_document(document)?;
    apply_encryption(
        &mut document,
        options.algorithm,
        &options.owner_password,
        &options.user_password,
        &options.permissions,
    )?;
    save_security_pdf(document, limits)
}

fn apply_encryption(
    document: &mut Document,
    algorithm: EncryptionAlgorithm,
    owner_password: &str,
    user_password: &str,
    policy: &PermissionPolicy,
) -> Result<(), OxideError> {
    let owner_password = Zeroizing::new(owner_password.to_owned());
    let user_password = Zeroizing::new(user_password.to_owned());
    let permissions = policy.to_lopdf_permissions();

    match algorithm {
        EncryptionAlgorithm::Aes256 => {
            let crypt_filter: Arc<dyn CryptFilter> = Arc::new(Aes256CryptFilter);
            let mut file_key = Zeroizing::new([0u8; 32]);
            rand::rng().fill(&mut *file_key);
            let version = EncryptionVersion::V5 {
                encrypt_metadata: true,
                crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), crypt_filter)]),
                file_encryption_key: &*file_key,
                stream_filter: b"StdCF".to_vec(),
                string_filter: b"StdCF".to_vec(),
                owner_password: &owner_password,
                user_password: &user_password,
                permissions,
            };
            let state = EncryptionState::try_from(version).map_err(map_lopdf_security_error)?;
            document.encrypt(&state).map_err(map_lopdf_security_error)
        }
        EncryptionAlgorithm::Aes128 => {
            let crypt_filter: Arc<dyn CryptFilter> = Arc::new(Aes128CryptFilter);
            let version = EncryptionVersion::V4 {
                document,
                encrypt_metadata: true,
                crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), crypt_filter)]),
                stream_filter: b"StdCF".to_vec(),
                string_filter: b"StdCF".to_vec(),
                owner_password: &owner_password,
                user_password: &user_password,
                permissions,
            };
            let state = EncryptionState::try_from(version).map_err(map_lopdf_security_error)?;
            document.encrypt(&state).map_err(map_lopdf_security_error)
        }
        EncryptionAlgorithm::Rc4 => Err(unsupported_rc4()),
    }
}

fn ensure_supported_algorithm(algorithm: EncryptionAlgorithm) -> Result<(), OxideError> {
    match algorithm {
        EncryptionAlgorithm::Aes128 | EncryptionAlgorithm::Aes256 => Ok(()),
        EncryptionAlgorithm::Rc4 => Err(unsupported_rc4()),
    }
}

fn ensure_explicit_passwords(owner_password: &str, user_password: &str) -> Result<(), OxideError> {
    if owner_password.is_empty() || user_password.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "owner_password and user_password must be explicitly set".to_owned(),
        });
    }
    Ok(())
}

fn required_password(password: Option<&str>) -> Result<Zeroizing<String>, OxideError> {
    match password {
        Some(password) if !password.is_empty() => Ok(Zeroizing::new(password.to_owned())),
        _ => Err(OxideError::EncryptedPdf),
    }
}

fn ensure_supported_encryption_revision(document: &Document) -> Result<(), OxideError> {
    let revision = encryption_revision(document)?;
    match revision {
        4 | 6 => Ok(()),
        2 | 3 => Err(unsupported_rc4()),
        5 => Err(OxideError::UnsupportedPdfFeature {
            feature: "proprietary Standard Security Handler revision 5".to_owned(),
        }),
        other => Err(OxideError::UnsupportedPdfFeature {
            feature: format!("Standard Security Handler revision {other}"),
        }),
    }
}

fn report_from_encrypted_document(document: &Document) -> Result<PermissionReport, OxideError> {
    let encrypted = document
        .get_encrypted()
        .map_err(|_| OxideError::EncryptedPdf)?;
    let version = encrypted.get(b"V").ok().and_then(object_i64);
    let revision = encrypted.get(b"R").ok().and_then(object_i64);
    let key_length_bits = encrypted.get(b"Length").ok().and_then(object_i64);
    let permissions_bits = encrypted.get(b"P").ok().and_then(object_i64);
    let handler = encrypted
        .get(b"Filter")
        .ok()
        .and_then(|object| object.as_name().ok())
        .map(|name| String::from_utf8_lossy(name).into_owned());

    Ok(PermissionReport {
        encrypted: true,
        handler,
        version,
        revision,
        key_length_bits,
        permissions_bits,
        permissions: permissions_bits
            .map(PermissionPolicy::from_bits)
            .unwrap_or_default(),
    })
}

fn encryption_revision(document: &Document) -> Result<i64, OxideError> {
    document
        .get_encrypted()
        .ok()
        .and_then(|dict| dict.get(b"R").ok())
        .and_then(object_i64)
        .ok_or_else(|| OxideError::UnsupportedPdfFeature {
            feature: "encrypted PDF without Standard Security Handler revision".to_owned(),
        })
}

fn object_i64(object: &Object) -> Option<i64> {
    match object {
        Object::Integer(value) => Some(*value),
        _ => None,
    }
}

fn save_security_pdf(
    mut document: Document,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .map_err(|_| OxideError::WritePdf)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

fn normalize_plain_document(mut document: Document) -> Result<Document, OxideError> {
    document.prune_objects();
    document.renumber_objects();
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .map_err(|_| OxideError::WritePdf)?;
    Document::load_mem(&bytes).map_err(|_| OxideError::ParsePdf)
}

fn load_decrypted_document(
    input: &[u8],
    encrypted_document: &Document,
    password: &str,
) -> Result<Document, OxideError> {
    encrypted_document
        .authenticate_password(password)
        .map_err(map_lopdf_security_error)?;
    let state =
        EncryptionState::decode(encrypted_document, password).map_err(map_lopdf_security_error)?;
    let encryption_object_id = encrypted_document
        .trailer
        .get(b"Encrypt")
        .ok()
        .and_then(|object| object.as_reference().ok());

    let mut document = encrypted_document.clone();
    document.objects.clear();

    // Decrypt every directly-addressed (uncompressed) object first. Object
    // stream containers are themselves Normal stream objects, so this also
    // decrypts the containers we need below.
    for (object_id, offset, generation) in normal_object_offsets(encrypted_document) {
        if Some(object_id) == encryption_object_id {
            continue;
        }
        let Some(raw) = raw_indirect_object(input, offset) else {
            return Err(OxideError::ParsePdf);
        };
        let mut object = parse_single_indirect_object(&raw, object_id, generation)?;
        lopdf::encryption::decrypt_object(&state, object_id, &mut object)
            .map_err(|error| map_lopdf_security_error(lopdf::Error::Decryption(error)))?;
        document.objects.insert(object_id, object);
    }

    // Now expand any compressed objects. Per PDF spec, objects inside an object
    // stream are not individually encrypted — the encryption applies to the
    // container stream as a whole, which the loop above already decrypted. So we
    // decode each container in place and lift its objects into the document.
    decode_compressed_objects(encrypted_document, &mut document)?;

    document.trailer.remove(b"Encrypt");
    document.encryption_state = None;
    Ok(document)
}

/// Lifts objects stored in object streams (`XrefEntry::Compressed`) into the
/// document by decoding each already-decrypted container stream.
pub(crate) fn decode_compressed_objects(
    encrypted_document: &Document,
    document: &mut Document,
) -> Result<(), OxideError> {
    use std::collections::BTreeSet;

    // Collect the container object numbers referenced by compressed entries.
    let containers = encrypted_document
        .reference_table
        .entries
        .values()
        .filter_map(|entry| match entry {
            XrefEntry::Compressed { container, .. } => Some(*container),
            _ => None,
        })
        .collect::<BTreeSet<_>>();

    for container in containers {
        let container_id = (container, 0);
        let Some(object) = document.objects.get_mut(&container_id) else {
            // Container missing from the decrypted set; nothing to expand.
            continue;
        };
        let stream = object.as_stream_mut().map_err(|_| OxideError::ParsePdf)?;
        let object_stream = lopdf::ObjectStream::new(stream).map_err(|_| OxideError::ParsePdf)?;
        for (id, decoded) in object_stream.objects {
            document.objects.entry(id).or_insert(decoded);
        }
    }

    Ok(())
}

fn normal_object_offsets(document: &Document) -> Vec<(lopdf::ObjectId, usize, u16)> {
    document
        .reference_table
        .entries
        .iter()
        .filter_map(|(&id, entry)| match *entry {
            XrefEntry::Normal { offset, generation } => {
                Some(((id, generation), offset as usize, generation))
            }
            _ => None,
        })
        .collect()
}

fn raw_indirect_object(input: &[u8], offset: usize) -> Option<Vec<u8>> {
    let slice = input.get(offset..)?;
    let end = slice
        .windows(b"endobj".len())
        .position(|window| window == b"endobj")?
        + b"endobj".len();
    Some(slice[..end].to_vec())
}

fn parse_single_indirect_object(
    raw: &[u8],
    object_id: lopdf::ObjectId,
    generation: u16,
) -> Result<Object, OxideError> {
    let header = b"%PDF-1.7\n";
    let object_offset = header.len();
    let xref_start = object_offset + raw.len() + 1;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(header);
    bytes.extend_from_slice(raw);
    bytes.extend_from_slice(b"\n");
    bytes.extend_from_slice(
        format!(
            "xref\n0 1\n0000000000 65535 f \n{} 1\n{:010} {:05} n \ntrailer\n<< /Size {} >>\nstartxref\n{}\n%%EOF\n",
            object_id.0,
            object_offset,
            generation,
            object_id.0 + 1,
            xref_start,
        )
        .as_bytes(),
    );
    let document = Document::load_mem(&bytes).map_err(|_| OxideError::ParsePdf)?;
    document
        .objects
        .get(&object_id)
        .cloned()
        .ok_or(OxideError::ParsePdf)
}

fn map_lopdf_security_error(error: lopdf::Error) -> OxideError {
    match error {
        lopdf::Error::Decryption(lopdf::encryption::DecryptionError::IncorrectPassword) => {
            OxideError::IncorrectPassword
        }
        lopdf::Error::Decryption(lopdf::encryption::DecryptionError::UnsupportedRevision) => {
            OxideError::UnsupportedPdfFeature {
                feature: "unsupported Standard Security Handler revision".to_owned(),
            }
        }
        lopdf::Error::Decryption(lopdf::encryption::DecryptionError::UnsupportedVersion) => {
            OxideError::UnsupportedPdfFeature {
                feature: "unsupported Standard Security Handler version".to_owned(),
            }
        }
        lopdf::Error::UnsupportedSecurityHandler(_) => OxideError::UnsupportedPdfFeature {
            feature: "unsupported security handler".to_owned(),
        },
        lopdf::Error::AlreadyEncrypted => OxideError::EncryptedPdf,
        lopdf::Error::NotEncrypted => OxideError::InvalidInput {
            reason: "PDF is not encrypted".to_owned(),
        },
        lopdf::Error::Decryption(_) => OxideError::EncryptedPdf,
        _ => OxideError::ParsePdf,
    }
}

fn unsupported_rc4() -> OxideError {
    OxideError::UnsupportedPdfFeature {
        feature: "RC4 Standard Security Handler".to_owned(),
    }
}
