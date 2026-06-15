mod types;
pub use types::*;

use crate::{
    OxideError, ResourceLimits, TextArtifact, enforce_input_bytes, enforce_max_pages,
    enforce_output_bytes, ensure_pdf_magic, load_pdf, save_pdf,
};
use cms::{
    builder::{SignedDataBuilder, SignerInfoBuilder},
    cert::CertificateChoices,
    content_info::ContentInfo,
    signed_data::{SignedAttributes, SignedData, SignerIdentifier},
};
use const_oid::AssociatedOid;
use der::{Decode as DerDecode, Encode};
use lopdf::{Dictionary, dictionary};
use p256::pkcs8::DecodePrivateKey;
use sha2::Digest;
use spki::AlgorithmIdentifierOwned;
use x509_cert::Certificate;
use x509_cert::ext::pkix::{BasicConstraints, KeyUsage};

include!("signatures/research.rs");
include!("signatures/entry.rs");
include!("signatures/add.rs");
include!("signatures/delete.rs");
include!("signatures/verify.rs");
