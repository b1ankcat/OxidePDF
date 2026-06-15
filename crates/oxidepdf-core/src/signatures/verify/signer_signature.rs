fn signer_certificate<'a>(
    certificates: &'a cms::signed_data::CertificateSet,
    sid: &SignerIdentifier,
) -> Option<&'a Certificate> {
    certificates.0.iter().find_map(|choice| {
        let CertificateChoices::Certificate(certificate) = choice else {
            return None;
        };
        match sid {
            SignerIdentifier::IssuerAndSerialNumber(issuer_and_serial) => {
                (certificate.tbs_certificate.issuer == issuer_and_serial.issuer
                    && certificate.tbs_certificate.serial_number == issuer_and_serial.serial_number)
                    .then_some(certificate)
            }
            SignerIdentifier::SubjectKeyIdentifier(_) => None,
        }
    })
}

fn signed_attributes_signature_input(signed_attrs: &SignedAttributes) -> Option<Vec<u8>> {
    let mut value = signed_attrs.to_der().ok()?;
    if let Some(first) = value.first_mut() {
        *first = 0x31;
        Some(value)
    } else {
        None
    }
}

fn verify_signer_signature(
    subject_public_key_info: &spki::SubjectPublicKeyInfoOwned,
    signature_algorithm_oid: &const_oid::ObjectIdentifier,
    digest_algorithm_oid: &const_oid::ObjectIdentifier,
    message: &[u8],
    signature: &[u8],
) -> SignatureCheckStatus {
    if subject_public_key_info.algorithm.oid == const_oid::db::rfc5912::RSA_ENCRYPTION {
        return verify_rsa_pkcs1v15_signature(
            subject_public_key_info,
            signature_algorithm_oid,
            digest_algorithm_oid,
            message,
            signature,
        );
    }
    if subject_public_key_info.algorithm.oid == const_oid::db::rfc5912::ID_EC_PUBLIC_KEY {
        return verify_ecdsa_p256_signature(
            subject_public_key_info,
            signature_algorithm_oid,
            digest_algorithm_oid,
            message,
            signature,
        );
    }

    signature_check(
        SignatureCheckState::Unsupported,
        format!(
            "unsupported signer public key algorithm {}",
            subject_public_key_info.algorithm.oid
        ),
    )
}

fn verify_ecdsa_p256_signature(
    subject_public_key_info: &spki::SubjectPublicKeyInfoOwned,
    signature_algorithm_oid: &const_oid::ObjectIdentifier,
    digest_algorithm_oid: &const_oid::ObjectIdentifier,
    message: &[u8],
    signature: &[u8],
) -> SignatureCheckStatus {
    // The signature algorithm must be ECDSA-with-SHA-256. The digest OID is the
    // bare id-sha256 when called from the CMS signed-attributes path, but the
    // combined ecdsa-with-SHA-256 OID when called from certificate-chain
    // verification (RFC 5280 ties tbsCertificate.signature to signatureAlgorithm).
    // Reject any other declared digest, e.g. SHA-384.
    let digest_is_sha256 = *digest_algorithm_oid == const_oid::db::rfc5912::ID_SHA_256
        || *digest_algorithm_oid == const_oid::db::rfc5912::ECDSA_WITH_SHA_256;
    if *signature_algorithm_oid != const_oid::db::rfc5912::ECDSA_WITH_SHA_256 || !digest_is_sha256 {
        return signature_check(
            SignatureCheckState::Unsupported,
            format!(
                "unsupported ECDSA signature algorithm {signature_algorithm_oid} with digest algorithm {digest_algorithm_oid}"
            ),
        );
    }

    let spki_der = match Encode::to_der(subject_public_key_info) {
        Ok(der) => der,
        Err(_) => {
            return signature_check(
                SignatureCheckState::Indeterminate,
                "signer ECDSA public key could not be re-encoded",
            );
        }
    };
    let verifying_key = match p256::ecdsa::VerifyingKey::from_public_key_der(&spki_der) {
        Ok(key) => key,
        Err(_) => {
            return signature_check(
                SignatureCheckState::Failed,
                "signer ECDSA P-256 public key could not be parsed",
            );
        }
    };
    let signature = match p256::ecdsa::DerSignature::from_der(signature) {
        Ok(signature) => signature,
        Err(_) => {
            return signature_check(
                SignatureCheckState::Failed,
                "ECDSA signature value could not be parsed",
            );
        }
    };

    use p256::pkcs8::DecodePublicKey;
    use signature::Verifier;

    if verifying_key.verify(message, &signature).is_ok() {
        signature_check(
            SignatureCheckState::Passed,
            "ECDSA P-256 signature mathematics verified",
        )
    } else {
        signature_check(
            SignatureCheckState::Failed,
            "ECDSA P-256 signature mathematics verification failed",
        )
    }
}

fn verify_rsa_pkcs1v15_signature(
    subject_public_key_info: &spki::SubjectPublicKeyInfoOwned,
    signature_algorithm_oid: &const_oid::ObjectIdentifier,
    digest_algorithm_oid: &const_oid::ObjectIdentifier,
    message: &[u8],
    signature: &[u8],
) -> SignatureCheckStatus {
    let spki_der = match Encode::to_der(subject_public_key_info) {
        Ok(der) => der,
        Err(_) => {
            return signature_check(
                SignatureCheckState::Indeterminate,
                "signer RSA public key could not be re-encoded",
            );
        }
    };
    let public_key = match rsa::RsaPublicKey::from_public_key_der(&spki_der) {
        Ok(public_key) => public_key,
        Err(_) => {
            return signature_check(
                SignatureCheckState::Failed,
                "signer RSA public key could not be parsed",
            );
        }
    };
    let signature = match rsa::pkcs1v15::Signature::try_from(signature) {
        Ok(signature) => signature,
        Err(_) => {
            return signature_check(
                SignatureCheckState::Failed,
                "RSA signature value could not be parsed",
            );
        }
    };

    use rsa::pkcs8::DecodePublicKey;
    use signature::Verifier;

    let verified = if *signature_algorithm_oid
        == const_oid::db::rfc5912::SHA_256_WITH_RSA_ENCRYPTION
        && *digest_algorithm_oid == const_oid::db::rfc5912::ID_SHA_256
    {
        rsa::pkcs1v15::VerifyingKey::<sha2::Sha256>::new(public_key).verify(message, &signature)
    } else if *signature_algorithm_oid == const_oid::db::rfc5912::SHA_384_WITH_RSA_ENCRYPTION
        && *digest_algorithm_oid == const_oid::db::rfc5912::ID_SHA_384
    {
        rsa::pkcs1v15::VerifyingKey::<sha2::Sha384>::new(public_key).verify(message, &signature)
    } else if *signature_algorithm_oid == const_oid::db::rfc5912::SHA_512_WITH_RSA_ENCRYPTION
        && *digest_algorithm_oid == const_oid::db::rfc5912::ID_SHA_512
    {
        rsa::pkcs1v15::VerifyingKey::<sha2::Sha512>::new(public_key).verify(message, &signature)
    } else {
        return signature_check(
            SignatureCheckState::Unsupported,
            format!(
                "unsupported RSA signature algorithm {signature_algorithm_oid} with digest algorithm {digest_algorithm_oid}"
            ),
        );
    };

    if verified.is_ok() {
        signature_check(
            SignatureCheckState::Passed,
            "RSA signature mathematics verified",
        )
    } else {
        signature_check(
            SignatureCheckState::Failed,
            "RSA signature mathematics verification failed",
        )
    }
}

fn message_digest_attribute(attribute: &x509_cert::attr::Attribute) -> Option<Vec<u8>> {
    if attribute.oid != const_oid::db::rfc5911::ID_MESSAGE_DIGEST {
        return None;
    }
    attribute
        .values
        .iter()
        .next()
        .and_then(|value| value.decode_as::<der::asn1::OctetString>().ok())
        .map(|value| value.as_bytes().to_vec())
}

fn digest_for_algorithm(oid: &const_oid::ObjectIdentifier, input: &[u8]) -> Option<Vec<u8>> {
    use sha2::{Digest, Sha256, Sha384, Sha512};

    if *oid == const_oid::db::rfc5912::ID_SHA_256 {
        return Some(Sha256::digest(input).to_vec());
    }
    if *oid == const_oid::db::rfc5912::ID_SHA_384 {
        return Some(Sha384::digest(input).to_vec());
    }
    if *oid == const_oid::db::rfc5912::ID_SHA_512 {
        return Some(Sha512::digest(input).to_vec());
    }

    None
}
