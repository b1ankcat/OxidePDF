
fn check_one_signer_signature(
    signer_info: &cms::signed_data::SignerInfo,
    certificates: &cms::signed_data::CertificateSet,
    signed_bytes: &[u8],
) -> SignatureCheckStatus {
    let Some(certificate) = signer_certificate(certificates, &signer_info.sid) else {
        return signature_check(
            SignatureCheckState::Failed,
            "CMS signer certificate was not found in embedded certificates",
        );
    };
    let signature_input = match signer_info.signed_attrs.as_ref() {
        Some(signed_attrs) => match signed_attributes_signature_input(signed_attrs) {
            Some(input) => input,
            None => return signature_check(
                SignatureCheckState::Indeterminate,
                "CMS signed attributes could not be re-encoded for signature verification",
            ),
        },
        None => signed_bytes.to_vec(),
    };
    verify_signer_signature(
        &certificate.tbs_certificate.subject_public_key_info,
        &signer_info.signature_algorithm.oid,
        &signer_info.digest_alg.oid,
        &signature_input,
        signer_info.signature.as_bytes(),
    )
}

fn cms_digest_verification(signed_data: &SignedData, signed_bytes: &[u8]) -> SignatureCheckStatus {
    signed_data
        .signer_infos
        .0
        .iter()
        .map(|si| check_one_signer_digest(si, signed_bytes))
        .reduce(worst_check)
        .unwrap_or_else(|| signature_check(SignatureCheckState::Failed, "CMS SignedData contains no signerInfo entries"))
}

fn cms_signature_verification(
    signed_data: &SignedData,
    signed_bytes: &[u8],
) -> SignatureCheckStatus {
    let Some(certificates) = signed_data.certificates.as_ref() else {
        return signature_check(
            SignatureCheckState::Failed,
            "CMS SignedData contains no embedded certificates",
        );
    };
    signed_data
        .signer_infos
        .0
        .iter()
        .map(|si| check_one_signer_signature(si, certificates, signed_bytes))
        .reduce(worst_check)
        .unwrap_or_else(|| signature_check(SignatureCheckState::Failed, "CMS SignedData contains no signerInfo entries"))
}
