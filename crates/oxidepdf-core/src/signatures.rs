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

#[cfg(test)]
mod tests {
    use super::*;
    use der::Decode;
    use rsa::pkcs8::EncodePublicKey;

    #[test]
    fn rsa_signature_verification_rejects_mismatched_signature_and_digest_oids() {
        let private_key =
            rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048).expect("key generation");
        let public_key = rsa::RsaPublicKey::from(&private_key);
        let public_key_der = public_key.to_public_key_der().expect("encode public key");
        let subject_public_key_info =
            spki::SubjectPublicKeyInfoOwned::from_der(public_key_der.as_bytes())
                .expect("parse public key");
        let signing_key = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key);
        let signature = rsa::signature::Signer::sign(&signing_key, b"message");
        let signature_bytes = rsa::signature::SignatureEncoding::to_vec(&signature);

        let status = verify_rsa_pkcs1v15_signature(
            &subject_public_key_info,
            &const_oid::db::rfc5912::SHA_256_WITH_RSA_ENCRYPTION,
            &const_oid::db::rfc5912::ID_SHA_512,
            b"message",
            &signature_bytes,
        );

        assert_eq!(status.status, SignatureCheckState::Unsupported);
    }
}
