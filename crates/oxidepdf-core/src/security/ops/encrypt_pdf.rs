use super::object_streams::decode_compressed_objects;
use super::types::{
    EncryptionAlgorithm, PermissionPolicy, PermissionReport, SecurityDecryptOptions,
    SecurityEncryptOptions, SecurityPermissionGetOptions, SecurityPermissionSetOptions,
};
use crate::{
    OxideError, PdfArtifact, ResourceLimits, TextArtifact, enforce_input_bytes, enforce_max_pages,
    enforce_output_bytes, ensure_pdf_magic,
};
use lopdf::encryption::crypt_filters::{Aes128CryptFilter, Aes256CryptFilter, CryptFilter};
use lopdf::xref::XrefEntry;
use lopdf::{Document, EncryptionState, EncryptionVersion, Object};
use rand::RngExt as _;
use std::collections::BTreeMap;
use std::sync::Arc;
use zeroize::Zeroizing;

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
    enforce_output_bytes(text.len(), limits)?;
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
