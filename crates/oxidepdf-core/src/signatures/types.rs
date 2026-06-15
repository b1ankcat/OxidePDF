use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Options for adding a digital signature.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SignatureAddOptions {
    /// Signature field name to create or fill.
    pub field_name: String,
    /// PEM file containing the signer certificate.
    pub certificate: PathBuf,
    /// PEM file containing the signer private key.
    pub private_key: PathBuf,
    /// Reserved signature Contents bytes.
    pub contents_reserved_bytes: Option<usize>,
    /// Optional visual signature appearance field to bind.
    pub appearance_field: Option<String>,
}

/// Options for deleting a signature field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SignatureDeleteFieldOptions {
    /// Signature field name to delete.
    pub field_name: String,
    /// Allow deleting a field that contains signature value material.
    pub destructive: bool,
}

/// Options for adding or reporting a timestamp token.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TimestampAddOptions {
    /// Explicit TSA endpoint. Live TSA requests are not performed by this offline build.
    pub tsa_url: Option<String>,
    /// Explicit RFC 3161 timestamp token DER file.
    pub token: Option<PathBuf>,
}

/// Timestamp operation report emitted as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimestampReport {
    /// Timestamp status.
    pub status: SignatureCheckStatus,
    /// Whether the input PDF bytes were preserved.
    pub input_preserved: bool,
    /// Non-sensitive diagnostics.
    pub diagnostics: Vec<SignatureDiagnostic>,
}

/// Options for signature and certificate operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SignatureOptions {
    /// Requested signature operation.
    pub mode: SignatureMode,
    /// PEM file containing explicit trust anchors for chain validation.
    pub trust_anchors: Option<PathBuf>,
}

/// Requested signature operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureMode {
    /// List PDF signatures without performing trust validation.
    List,
    /// Verify PDF signatures and embedded certificate material.
    #[default]
    Verify,
}

/// Top-level signature verification report emitted as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureVerificationReport {
    /// Overall verification verdict.
    pub verdict: SignatureVerdict,
    /// Number of trust anchors accepted from the explicit PEM input.
    pub trust_anchor_count: usize,
    /// Per-signature reports.
    pub signatures: Vec<SignatureEntryReport>,
    /// Top-level diagnostics.
    pub diagnostics: Vec<SignatureDiagnostic>,
}

/// Top-level signature listing report emitted as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureListReport {
    /// Per-signature structural summaries.
    pub signatures: Vec<SignatureListEntry>,
    /// Top-level diagnostics.
    pub diagnostics: Vec<SignatureDiagnostic>,
}

/// Per-signature list entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureListEntry {
    /// Optional field name from the PDF form tree.
    pub field_name: Option<String>,
    /// Signature dictionary SubFilter value.
    pub subfilter: Option<String>,
    /// ByteRange structural status.
    pub byte_range: ByteRangeVerification,
    /// Contents coverage status.
    pub contents: ContentsVerification,
}

/// Stable signature verification verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureVerdict {
    /// All required checks completed and the signature chains to a trust anchor.
    Trusted,
    /// At least one completed check proved the signature invalid.
    Invalid,
    /// Verification completed but trust could not be established offline.
    Indeterminate,
    /// The input uses a signature feature not supported by this build.
    Unsupported,
}

/// Per-signature report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureEntryReport {
    /// Optional field name from the PDF form tree.
    pub field_name: Option<String>,
    /// Signature dictionary SubFilter value.
    pub subfilter: Option<String>,
    /// ByteRange structural status.
    pub byte_range: ByteRangeVerification,
    /// Contents coverage status.
    pub contents: ContentsVerification,
    /// CMS parse/validation status.
    pub cms_status: SignatureCheckStatus,
    /// Signed content digest status.
    pub digest_status: SignatureCheckStatus,
    /// Signer signature mathematics status.
    pub signature_status: SignatureCheckStatus,
    /// Certificate chain status.
    pub certificate_chain_status: SignatureCheckStatus,
    /// Offline revocation status.
    pub revocation_status: SignatureCheckStatus,
    /// Timestamp token validation status.
    pub timestamp_status: SignatureCheckStatus,
    /// Per-signature diagnostics.
    pub diagnostics: Vec<SignatureDiagnostic>,
}

/// ByteRange check result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRangeVerification {
    /// Parsed ByteRange values.
    pub values: Option<[u64; 4]>,
    /// Whether the ranges are in input bounds.
    pub in_bounds: bool,
    /// Whether the ranges are ordered and non-overlapping.
    pub ordered_non_overlapping: bool,
    /// Length of the unsigned gap between signed ranges.
    pub gap_len: Option<u64>,
    /// Total covered bytes.
    pub covered_len: Option<u64>,
}

/// Contents coverage check result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentsVerification {
    /// Number of bytes in the signature Contents value.
    pub byte_len: Option<usize>,
    /// Whether the ByteRange gap can contain the Contents placeholder.
    pub covered_by_gap: bool,
}

/// Status for an individual signature check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureCheckStatus {
    /// Stable status code.
    pub status: SignatureCheckState,
    /// Non-sensitive detail.
    pub detail: String,
}

/// Stable signature check state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureCheckState {
    /// Check completed successfully.
    Passed,
    /// Check completed and failed.
    Failed,
    /// Check could not establish a definite result.
    Indeterminate,
    /// Check is not implemented in this build.
    Unsupported,
}

/// Non-sensitive signature diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureDiagnostic {
    /// Stable diagnostic code.
    pub code: String,
    /// Non-sensitive diagnostic message.
    pub message: String,
}

/// Non-production report returned by the signature research scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureResearchReport {
    /// Count of literal `/Type /Sig` markers.
    pub signature_dictionary_count: usize,
    /// Parsed `/ByteRange [...]` arrays found by the scanner.
    pub byte_ranges: Vec<ByteRangeResearch>,
    /// Literal `/SubFilter /Name` values seen in the PDF bytes.
    pub subfilters: Vec<String>,
}

/// Non-production structural summary of a PDF signature ByteRange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteRangeResearch {
    /// First signed range start offset.
    pub first_start: u64,
    /// First signed range length.
    pub first_len: u64,
    /// Second signed range start offset.
    pub second_start: u64,
    /// Second signed range length.
    pub second_len: u64,
    /// Whether both ranges are inside the input byte length.
    pub in_bounds: bool,
    /// Whether the two ranges are non-overlapping and ordered.
    pub ordered_non_overlapping: bool,
    /// Length of the unsigned gap between the two signed ranges, if ordered.
    pub gap_len: Option<u64>,
    /// Total number of bytes covered by the two signed ranges.
    pub covered_len: Option<u64>,
}
