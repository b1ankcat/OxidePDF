use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{
    enforce_input_bytes, enforce_max_pages, enforce_output_bytes, ensure_pdf_magic, load_pdf,
    OxideError, ResourceLimits, TextArtifact,
};
use lopdf::Dictionary;
use x509_cert::{der::Decode, Certificate};

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

/// Scans PDF bytes for signature dictionary markers for research prototypes.
///
/// This does not cryptographically verify signatures, certificates, digests,
/// revocation data, timestamp tokens, or PAdES policy. Until the formal
/// signature implementation is added, production workflows must use
/// `PdfSignOptions::Verify`, which returns a structured unsupported status
/// for checks that are not implemented in this verification slice.
pub fn inspect_pdf_signature_markers_for_research(
    input: &[u8],
) -> Result<SignatureResearchReport, OxideError> {
    ensure_pdf_magic(input)?;

    Ok(SignatureResearchReport {
        signature_dictionary_count: count_subslice(input, b"/Type /Sig"),
        byte_ranges: parse_byte_ranges_for_research(input),
        subfilters: parse_name_values_after_token(input, b"/SubFilter"),
    })
}

/// Verifies PDF signature structure and emits a JSON report.
pub fn verify_pdf_signatures(
    input: &[u8],
    options: &SignatureOptions,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let trust_anchor_count = trust_anchor_count(options.trust_anchors.as_deref())?;
    let document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    let mut diagnostics = Vec::new();
    let signatures = discover_pdf_signature_dictionaries(&document)?
        .into_iter()
        .map(|dictionary| signature_entry_report(input, dictionary))
        .collect::<Vec<_>>();

    if signatures.is_empty() {
        diagnostics.push(signature_diagnostic(
            "no_signatures",
            "PDF contains no signature dictionaries",
        ));
    }

    let verdict = overall_signature_verdict(&signatures, &diagnostics);
    let report = SignatureVerificationReport {
        verdict,
        trust_anchor_count,
        signatures,
        diagnostics,
    };
    let text = serde_json::to_string_pretty(&report).map_err(|_| OxideError::Internal)?;
    enforce_output_bytes(text.len(), limits)?;

    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

#[derive(Debug, Clone)]
struct DiscoveredSignatureDictionary<'a> {
    field_name: Option<String>,
    dictionary: &'a Dictionary,
}

fn trust_anchor_count(path: Option<&std::path::Path>) -> Result<usize, OxideError> {
    let path = path.ok_or_else(|| OxideError::InvalidInput {
        reason: "signature verification requires explicit trust anchors".to_owned(),
    })?;
    let pem = std::fs::read(path).map_err(|_| OxideError::Io)?;
    let pem = std::str::from_utf8(&pem).map_err(|_| OxideError::InvalidInput {
        reason: "trust anchors file contains no valid PEM certificates".to_owned(),
    })?;
    let count = parsed_trust_anchor_count(pem)?;
    if count == 0 {
        return Err(OxideError::InvalidInput {
            reason: "trust anchors file contains no valid PEM certificates".to_owned(),
        });
    }

    Ok(count)
}

fn parsed_trust_anchor_count(pem: &str) -> Result<usize, OxideError> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";

    let mut rest = pem;
    let mut count = 0usize;
    while let Some(begin) = rest.find(BEGIN) {
        rest = &rest[begin..];
        let Some(end) = rest.find(END) else {
            return Err(OxideError::InvalidInput {
                reason: "trust anchors file contains no valid PEM certificates".to_owned(),
            });
        };
        let block_end = end + END.len();
        let block = &rest[..block_end];
        let (label, der) =
            pem_rfc7468::decode_vec(block.as_bytes()).map_err(|_| OxideError::InvalidInput {
                reason: "trust anchors file contains no valid PEM certificates".to_owned(),
            })?;
        if label != "CERTIFICATE" {
            return Err(OxideError::InvalidInput {
                reason: "trust anchors file contains no valid PEM certificates".to_owned(),
            });
        }
        Certificate::from_der(&der).map_err(|_| OxideError::InvalidInput {
            reason: "trust anchors file contains no valid PEM certificates".to_owned(),
        })?;
        count += 1;
        rest = &rest[block_end..];
    }

    Ok(count)
}

fn discover_pdf_signature_dictionaries(
    document: &lopdf::Document,
) -> Result<Vec<DiscoveredSignatureDictionary<'_>>, OxideError> {
    let mut signatures = Vec::new();
    if let Ok(catalog) = document.catalog() {
        if let Ok(acroform) = catalog
            .get(b"AcroForm")
            .and_then(|object| deref_dictionary(document, object))
        {
            if let Ok(fields) = acroform.get(b"Fields").and_then(lopdf::Object::as_array) {
                for field in fields {
                    collect_signature_fields(document, field, None, &mut signatures)?;
                }
            }
        }
    }
    for (_, page_id) in document.get_pages() {
        let Ok(page) = document
            .get_object(page_id)
            .and_then(lopdf::Object::as_dict)
        else {
            continue;
        };
        let Ok(annots) = page.get(b"Annots").and_then(lopdf::Object::as_array) else {
            continue;
        };
        for annot in annots {
            collect_signature_fields(document, annot, None, &mut signatures)?;
        }
    }
    signatures.dedup_by_key(|signature| std::ptr::from_ref(signature.dictionary) as usize);

    Ok(signatures)
}

fn collect_signature_fields<'a>(
    document: &'a lopdf::Document,
    object: &'a lopdf::Object,
    inherited_name: Option<String>,
    signatures: &mut Vec<DiscoveredSignatureDictionary<'a>>,
) -> Result<(), OxideError> {
    let dictionary = deref_dictionary(document, object).map_err(|_| OxideError::ParsePdf)?;
    let field_name = dictionary
        .get(b"T")
        .ok()
        .and_then(pdf_string)
        .or(inherited_name);
    if dictionary.get(b"FT").and_then(lopdf::Object::as_name).ok() == Some(b"Sig") {
        if let Ok(value) = dictionary.get(b"V") {
            if let Ok(signature_dictionary) = deref_dictionary(document, value) {
                signatures.push(DiscoveredSignatureDictionary {
                    field_name: field_name.clone(),
                    dictionary: signature_dictionary,
                });
            }
        } else if dictionary.get(b"ByteRange").is_ok() {
            signatures.push(DiscoveredSignatureDictionary {
                field_name: field_name.clone(),
                dictionary,
            });
        }
    } else if dictionary.get(b"ByteRange").is_ok() && dictionary.get(b"Contents").is_ok() {
        signatures.push(DiscoveredSignatureDictionary {
            field_name: field_name.clone(),
            dictionary,
        });
    }
    if let Ok(kids) = dictionary.get(b"Kids").and_then(lopdf::Object::as_array) {
        for kid in kids {
            collect_signature_fields(document, kid, field_name.clone(), signatures)?;
        }
    }

    Ok(())
}

fn deref_dictionary<'a>(
    document: &'a lopdf::Document,
    object: &'a lopdf::Object,
) -> lopdf::Result<&'a Dictionary> {
    match object {
        lopdf::Object::Reference(id) => document.get_object(*id).and_then(lopdf::Object::as_dict),
        lopdf::Object::Dictionary(dictionary) => Ok(dictionary),
        _ => object.as_dict(),
    }
}

fn signature_entry_report(
    input: &[u8],
    discovered: DiscoveredSignatureDictionary<'_>,
) -> SignatureEntryReport {
    let mut diagnostics = Vec::new();
    let subfilter = discovered
        .dictionary
        .get(b"SubFilter")
        .ok()
        .and_then(pdf_name);
    if !matches!(
        subfilter.as_deref(),
        Some("adbe.pkcs7.detached") | Some("ETSI.CAdES.detached")
    ) {
        diagnostics.push(signature_diagnostic(
            "unsupported_subfilter",
            "signature SubFilter is not supported",
        ));
    }

    let byte_range = byte_range_verification(input, discovered.dictionary, &mut diagnostics);
    let contents = contents_verification(discovered.dictionary, &byte_range, &mut diagnostics);

    SignatureEntryReport {
        field_name: discovered.field_name,
        subfilter,
        byte_range,
        contents,
        cms_status: signature_check(
            SignatureCheckState::Unsupported,
            "CMS SignedData parsing is not implemented in this verification slice",
        ),
        digest_status: signature_check(
            SignatureCheckState::Indeterminate,
            "signed byte digest is not checked until CMS messageDigest parsing is available",
        ),
        signature_status: signature_check(
            SignatureCheckState::Unsupported,
            "signer signature mathematics is not implemented in this verification slice",
        ),
        certificate_chain_status: signature_check(
            SignatureCheckState::Indeterminate,
            "certificate chain cannot be trusted until signer certificate extraction is available",
        ),
        revocation_status: signature_check(
            SignatureCheckState::Indeterminate,
            "offline revocation status is not confirmed; no network lookup is performed",
        ),
        timestamp_status: signature_check(
            SignatureCheckState::Unsupported,
            "timestamp token validation is not implemented in this verification slice",
        ),
        diagnostics,
    }
}

fn byte_range_verification(
    input: &[u8],
    dictionary: &Dictionary,
    diagnostics: &mut Vec<SignatureDiagnostic>,
) -> ByteRangeVerification {
    let Some(values) = dictionary
        .get(b"ByteRange")
        .ok()
        .and_then(byte_range_values)
    else {
        diagnostics.push(signature_diagnostic(
            "missing_byte_range",
            "signature dictionary is missing ByteRange",
        ));
        return ByteRangeVerification {
            values: None,
            in_bounds: false,
            ordered_non_overlapping: false,
            gap_len: None,
            covered_len: None,
        };
    };
    let research = byte_range_research(
        values[0],
        values[1],
        values[2],
        values[3],
        input.len() as u64,
    );
    if !research.in_bounds {
        diagnostics.push(signature_diagnostic(
            "byte_range_out_of_bounds",
            "ByteRange references bytes outside the input",
        ));
    }
    if !research.ordered_non_overlapping {
        diagnostics.push(signature_diagnostic(
            "byte_range_not_ordered",
            "ByteRange entries are not ordered and non-overlapping",
        ));
    }

    ByteRangeVerification {
        values: Some(values),
        in_bounds: research.in_bounds,
        ordered_non_overlapping: research.ordered_non_overlapping,
        gap_len: research.gap_len,
        covered_len: research.covered_len,
    }
}

fn contents_verification(
    dictionary: &Dictionary,
    byte_range: &ByteRangeVerification,
    diagnostics: &mut Vec<SignatureDiagnostic>,
) -> ContentsVerification {
    let Some(byte_len) = dictionary
        .get(b"Contents")
        .ok()
        .and_then(pdf_string_bytes_len)
    else {
        diagnostics.push(signature_diagnostic(
            "missing_contents",
            "signature dictionary is missing Contents",
        ));
        return ContentsVerification {
            byte_len: None,
            covered_by_gap: false,
        };
    };
    let covered_by_gap = byte_range
        .gap_len
        .is_some_and(|gap_len| gap_len >= byte_len as u64);
    if !covered_by_gap {
        diagnostics.push(signature_diagnostic(
            "contents_not_covered_by_gap",
            "signature Contents is larger than the unsigned ByteRange gap",
        ));
    }

    ContentsVerification {
        byte_len: Some(byte_len),
        covered_by_gap,
    }
}

fn byte_range_values(object: &lopdf::Object) -> Option<[u64; 4]> {
    let array = object.as_array().ok()?;
    if array.len() != 4 {
        return None;
    }
    let mut values = [0u64; 4];
    for (index, object) in array.iter().enumerate() {
        let value = object.as_i64().ok()?;
        values[index] = u64::try_from(value).ok()?;
    }

    Some(values)
}

fn pdf_name(object: &lopdf::Object) -> Option<String> {
    object
        .as_name()
        .ok()
        .map(|name| String::from_utf8_lossy(name).into_owned())
}

fn pdf_string(object: &lopdf::Object) -> Option<String> {
    object
        .as_str()
        .ok()
        .map(|value| String::from_utf8_lossy(value).into_owned())
}

fn pdf_string_bytes_len(object: &lopdf::Object) -> Option<usize> {
    object.as_str().ok().map(<[u8]>::len)
}

fn signature_check(status: SignatureCheckState, detail: impl Into<String>) -> SignatureCheckStatus {
    SignatureCheckStatus {
        status,
        detail: detail.into(),
    }
}

fn signature_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
) -> SignatureDiagnostic {
    SignatureDiagnostic {
        code: code.into(),
        message: message.into(),
    }
}

fn overall_signature_verdict(
    signatures: &[SignatureEntryReport],
    diagnostics: &[SignatureDiagnostic],
) -> SignatureVerdict {
    if signatures.is_empty()
        || diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "no_signatures")
    {
        return SignatureVerdict::Indeterminate;
    }
    if signatures.iter().any(|signature| {
        signature
            .diagnostics
            .iter()
            .any(is_invalid_signature_diagnostic)
    }) {
        return SignatureVerdict::Invalid;
    }
    if signatures.iter().any(|signature| {
        signature.cms_status.status == SignatureCheckState::Unsupported
            || signature.signature_status.status == SignatureCheckState::Unsupported
            || signature.timestamp_status.status == SignatureCheckState::Unsupported
    }) {
        return SignatureVerdict::Unsupported;
    }

    SignatureVerdict::Indeterminate
}

fn is_invalid_signature_diagnostic(diagnostic: &SignatureDiagnostic) -> bool {
    matches!(
        diagnostic.code.as_str(),
        "missing_byte_range"
            | "byte_range_out_of_bounds"
            | "byte_range_not_ordered"
            | "missing_contents"
            | "contents_not_covered_by_gap"
    )
}

fn count_subslice(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() {
        return 0;
    }

    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn parse_byte_ranges_for_research(input: &[u8]) -> Vec<ByteRangeResearch> {
    let mut ranges = Vec::new();
    let mut offset = 0usize;
    while let Some(relative) = find_subslice(&input[offset..], b"/ByteRange") {
        let token_start = offset + relative;
        offset = token_start + b"/ByteRange".len();
        let Some(open_relative) = input[offset..].iter().position(|byte| *byte == b'[') else {
            continue;
        };
        let array_start = offset + open_relative + 1;
        let Some(close_relative) = input[array_start..].iter().position(|byte| *byte == b']')
        else {
            continue;
        };
        let array_end = array_start + close_relative;
        offset = array_end + 1;
        let numbers = input[array_start..array_end]
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|part| !part.is_empty())
            .filter_map(parse_ascii_u64)
            .collect::<Vec<_>>();
        if numbers.len() < 4 {
            continue;
        }
        ranges.push(byte_range_research(
            numbers[0],
            numbers[1],
            numbers[2],
            numbers[3],
            input.len() as u64,
        ));
    }

    ranges
}

fn byte_range_research(
    first_start: u64,
    first_len: u64,
    second_start: u64,
    second_len: u64,
    input_len: u64,
) -> ByteRangeResearch {
    let first_end = first_start.checked_add(first_len);
    let second_end = second_start.checked_add(second_len);
    let in_bounds = first_end.is_some_and(|end| end <= input_len)
        && second_end.is_some_and(|end| end <= input_len);
    let ordered_non_overlapping = first_end.is_some_and(|end| end <= second_start);
    let gap_len = ordered_non_overlapping.then(|| second_start - first_end.unwrap_or(second_start));
    let covered_len = first_len.checked_add(second_len);

    ByteRangeResearch {
        first_start,
        first_len,
        second_start,
        second_len,
        in_bounds,
        ordered_non_overlapping,
        gap_len,
        covered_len,
    }
}

fn parse_name_values_after_token(input: &[u8], token: &[u8]) -> Vec<String> {
    let mut values = Vec::new();
    let mut offset = 0usize;
    while let Some(relative) = find_subslice(&input[offset..], token) {
        let value_start = offset + relative + token.len();
        offset = value_start;
        let Some(name_start_relative) = input[value_start..].iter().position(|byte| *byte == b'/')
        else {
            continue;
        };
        let name_start = value_start + name_start_relative + 1;
        let name_end = input[name_start..]
            .iter()
            .position(|byte| is_pdf_delimiter_or_whitespace(*byte))
            .map_or(input.len(), |end| name_start + end);
        if name_end > name_start {
            values.push(String::from_utf8_lossy(&input[name_start..name_end]).into_owned());
        }
        offset = name_end;
    }

    values
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_ascii_u64(input: &[u8]) -> Option<u64> {
    if input.is_empty() || input.iter().any(|byte| !byte.is_ascii_digit()) {
        return None;
    }

    input.iter().try_fold(0u64, |acc, byte| {
        acc.checked_mul(10)?.checked_add(u64::from(byte - b'0'))
    })
}

fn is_pdf_delimiter_or_whitespace(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b'/' | b'<' | b'>' | b'[' | b']' | b'(' | b')')
}
