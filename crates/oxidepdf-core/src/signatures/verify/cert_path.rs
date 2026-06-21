fn cms_certificate_chain_status(
    signed_data: &SignedData,
    trust_anchors: &TrustAnchors,
) -> SignatureCheckStatus {
    if trust_anchors.certificates.is_empty() {
        return signature_check(
            SignatureCheckState::Indeterminate,
            "no explicit trust anchors were provided",
        );
    }
    let Some(certificates) = signed_data.certificates.as_ref() else {
        return signature_check(
            SignatureCheckState::Failed,
            "CMS SignedData contains no embedded certificates",
        );
    };
    // Validate the chain for every signerInfo, not just the first: a multi-signer
    // CMS must have *all* signers chain to a trust anchor before the overall
    // verdict can be trusted. The worst per-signer status wins.
    signed_data
        .signer_infos
        .0
        .iter()
        .map(|signer_info| one_signer_chain_status(signer_info, certificates, trust_anchors))
        .reduce(worst_check)
        .unwrap_or_else(|| {
            signature_check(
                SignatureCheckState::Failed,
                "CMS SignedData contains no signerInfo entries",
            )
        })
}

fn one_signer_chain_status(
    signer_info: &cms::signed_data::SignerInfo,
    certificates: &cms::signed_data::CertificateSet,
    trust_anchors: &TrustAnchors,
) -> SignatureCheckStatus {
    let Some(signer_certificate) = signer_certificate(certificates, &signer_info.sid) else {
        return signature_check(
            SignatureCheckState::Failed,
            "CMS signer certificate was not found in embedded certificates",
        );
    };
    match verify_certificate_path(signer_certificate, certificates, trust_anchors) {
        CertificatePathStatus::ChainsToTrustAnchor => signature_check(
            SignatureCheckState::Passed,
            "signer certificate chain signatures validate to an explicit trust anchor",
        ),
        CertificatePathStatus::InvalidSignature => signature_check(
            SignatureCheckState::Failed,
            "certificate chain signature verification failed",
        ),
        CertificatePathStatus::Expired => signature_check(
            SignatureCheckState::Failed,
            "certificate in the chain is outside its validity period",
        ),
        CertificatePathStatus::UnsupportedAlgorithm(oid) => signature_check(
            SignatureCheckState::Unsupported,
            format!("unsupported certificate signature algorithm {oid}"),
        ),
        CertificatePathStatus::NoIssuer => signature_check(
            SignatureCheckState::Indeterminate,
            "certificate chain does not reach an explicit trust anchor",
        ),
        CertificatePathStatus::UntrustedIssuer => signature_check(
            SignatureCheckState::Failed,
            "certificate chain relies on an issuer that is not a valid CA",
        ),
    }
}

enum CertificatePathStatus {
    ChainsToTrustAnchor,
    InvalidSignature,
    Expired,
    UnsupportedAlgorithm(const_oid::ObjectIdentifier),
    NoIssuer,
    UntrustedIssuer,
}

fn verify_certificate_path(
    signer_certificate: &Certificate,
    certificates: &cms::signed_data::CertificateSet,
    trust_anchors: &TrustAnchors,
) -> CertificatePathStatus {
    let now = std::time::SystemTime::now();
    let mut current = signer_certificate;
    // Number of intermediate CA certificates traversed so far, used to enforce
    // each issuer's pathLenConstraint.
    let mut intermediates_seen = 0u32;
    for _ in 0..=certificates.0.len() {
        if !certificate_is_time_valid(current, now) {
            return CertificatePathStatus::Expired;
        }
        if let Some(anchor) = trust_anchors
            .certificates
            .iter()
            .find(|anchor| current.tbs_certificate.issuer == anchor.tbs_certificate.subject)
        {
            // A trust anchor signing `current` must itself be a usable CA, and
            // its pathLenConstraint must allow the intermediates below it.
            if !issuer_is_valid_ca(anchor, intermediates_seen) {
                return CertificatePathStatus::UntrustedIssuer;
            }
            if !certificate_is_time_valid(anchor, now) {
                return CertificatePathStatus::Expired;
            }
            return match verify_certificate_signature(current, anchor) {
                SignatureCheckState::Passed => CertificatePathStatus::ChainsToTrustAnchor,
                SignatureCheckState::Failed => CertificatePathStatus::InvalidSignature,
                SignatureCheckState::Unsupported => {
                    CertificatePathStatus::UnsupportedAlgorithm(current.signature_algorithm.oid)
                }
                SignatureCheckState::Indeterminate => CertificatePathStatus::NoIssuer,
            };
        }

        let Some(issuer) = certificates.0.iter().find_map(|choice| {
            let CertificateChoices::Certificate(candidate) = choice else {
                return None;
            };
            (current.tbs_certificate.issuer == candidate.tbs_certificate.subject)
                .then_some(candidate)
        }) else {
            return CertificatePathStatus::NoIssuer;
        };
        if issuer.tbs_certificate.serial_number == current.tbs_certificate.serial_number
            && issuer.tbs_certificate.subject == current.tbs_certificate.subject
        {
            return CertificatePathStatus::NoIssuer;
        }
        // An embedded certificate may only act as an issuer if it is a CA whose
        // basicConstraints/keyUsage/pathLenConstraint permit signing `current`.
        // This blocks using an end-entity (e.g. TLS leaf) certificate to forge
        // a child certificate.
        if !issuer_is_valid_ca(issuer, intermediates_seen) {
            return CertificatePathStatus::UntrustedIssuer;
        }
        match verify_certificate_signature(current, issuer) {
            SignatureCheckState::Passed => {
                current = issuer;
                intermediates_seen = intermediates_seen.saturating_add(1);
            }
            SignatureCheckState::Failed => return CertificatePathStatus::InvalidSignature,
            SignatureCheckState::Unsupported => {
                return CertificatePathStatus::UnsupportedAlgorithm(
                    current.signature_algorithm.oid,
                );
            }
            SignatureCheckState::Indeterminate => return CertificatePathStatus::NoIssuer,
        }
    }

    CertificatePathStatus::NoIssuer
}

/// Returns true if `issuer` may sign certificates for a path with
/// `intermediates_below` intermediate CAs beneath it. Requires basicConstraints
/// `cA = true`; if a keyUsage extension is present it must allow `keyCertSign`;
/// and any pathLenConstraint must accommodate the intermediates below.
fn issuer_is_valid_ca(issuer: &Certificate, intermediates_below: u32) -> bool {
    let Some(extensions) = issuer.tbs_certificate.extensions.as_ref() else {
        // Without basicConstraints we cannot confirm CA status; reject rather
        // than assume authority.
        return false;
    };

    let mut is_ca = false;
    let mut path_len: Option<u8> = None;
    let mut key_cert_sign_ok = true;
    for extension in extensions {
        if extension.extn_id == BasicConstraints::OID {
            let Ok(basic) = BasicConstraints::from_der(extension.extn_value.as_bytes()) else {
                return false;
            };
            is_ca = basic.ca;
            path_len = basic.path_len_constraint;
        } else if extension.extn_id == KeyUsage::OID {
            match KeyUsage::from_der(extension.extn_value.as_bytes()) {
                Ok(key_usage) => key_cert_sign_ok = key_usage.key_cert_sign(),
                Err(_) => return false,
            }
        }
    }

    if !is_ca || !key_cert_sign_ok {
        return false;
    }
    match path_len {
        Some(max) => u32::from(max) >= intermediates_below,
        None => true,
    }
}

/// Returns true if `now` falls within the certificate's notBefore..notAfter
/// validity window. Expired or not-yet-valid certificates are rejected.
fn certificate_is_time_valid(certificate: &Certificate, now: std::time::SystemTime) -> bool {
    let validity = &certificate.tbs_certificate.validity;
    let not_before = validity.not_before.to_system_time();
    let not_after = validity.not_after.to_system_time();
    now >= not_before && now <= not_after
}

fn verify_certificate_signature(
    certificate: &Certificate,
    issuer: &Certificate,
) -> SignatureCheckState {
    let Ok(tbs_der) = certificate.tbs_certificate.to_der() else {
        return SignatureCheckState::Indeterminate;
    };
    let Some(signature) = certificate.signature.as_bytes() else {
        return SignatureCheckState::Unsupported;
    };
    let status = verify_signer_signature(
        &issuer.tbs_certificate.subject_public_key_info,
        &certificate.signature_algorithm.oid,
        &certificate.tbs_certificate.signature.oid,
        &tbs_der,
        signature,
    );

    status.status
}
