
fn build_p256_cms_signature(
    signed_bytes: &[u8],
    certificate: &Certificate,
    signing_key: &p256::ecdsa::SigningKey,
) -> Result<Vec<u8>, OxideError> {
    let digest = sha2::Sha256::digest(signed_bytes);
    let encap_content_info = cms::signed_data::EncapsulatedContentInfo {
        econtent_type: const_oid::db::rfc5911::ID_DATA,
        econtent: None,
    };
    let sid = SignerIdentifier::IssuerAndSerialNumber(cms::cert::IssuerAndSerialNumber {
        issuer: certificate.tbs_certificate.issuer.clone(),
        serial_number: certificate.tbs_certificate.serial_number.clone(),
    });
    let digest_algorithm = AlgorithmIdentifierOwned {
        oid: const_oid::db::rfc5912::ID_SHA_256,
        parameters: None,
    };
    let signer_info = SignerInfoBuilder::new(
        signing_key,
        sid,
        digest_algorithm.clone(),
        &encap_content_info,
        Some(&digest),
    )
    .map_err(|_| OxideError::InvalidInput {
        reason: "CMS signer info could not be prepared".to_owned(),
    })?;
    let mut signed_data = SignedDataBuilder::new(&encap_content_info);
    signed_data
        .add_digest_algorithm(digest_algorithm)
        .map_err(|_| OxideError::InvalidInput {
            reason: "CMS digest algorithm could not be added".to_owned(),
        })?;
    signed_data
        .add_certificate(CertificateChoices::Certificate(certificate.clone()))
        .map_err(|_| OxideError::InvalidInput {
            reason: "CMS certificate could not be added".to_owned(),
        })?;
    signed_data
        .add_signer_info::<p256::ecdsa::SigningKey, p256::ecdsa::DerSignature>(signer_info)
        .map_err(|_| OxideError::InvalidInput {
            reason: "CMS signer info could not be signed".to_owned(),
        })?;
    let content_info = signed_data.build().map_err(|_| OxideError::InvalidInput {
        reason: "CMS SignedData could not be encoded".to_owned(),
    })?;
    content_info.to_der().map_err(|_| OxideError::InvalidInput {
        reason: "CMS ContentInfo could not be encoded".to_owned(),
    })
}

fn load_signing_certificate(path: &std::path::Path) -> Result<Certificate, OxideError> {
    let pem = std::fs::read(path).map_err(|_| OxideError::Io)?;
    let pem = std::str::from_utf8(&pem).map_err(|_| OxideError::InvalidInput {
        reason: "signing certificate file contains no valid PEM certificate".to_owned(),
    })?;
    let certificates = parsed_trust_anchors(pem)?;
    certificates
        .into_iter()
        .next()
        .ok_or_else(|| OxideError::InvalidInput {
            reason: "signing certificate file contains no valid PEM certificate".to_owned(),
        })
}

fn load_p256_signing_key(path: &std::path::Path) -> Result<p256::ecdsa::SigningKey, OxideError> {
    let pem = zeroize::Zeroizing::new(std::fs::read_to_string(path).map_err(|_| OxideError::Io)?);
    p256::ecdsa::SigningKey::from_pkcs8_pem(&pem).map_err(|_| OxideError::InvalidInput {
        reason: "private key file must contain an unencrypted P-256 PKCS#8 PEM key".to_owned(),
    })
}
