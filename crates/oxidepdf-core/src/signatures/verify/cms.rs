struct CmsVerificationReport {
    cms_status: SignatureCheckStatus,
    digest_status: SignatureCheckStatus,
    signature_status: SignatureCheckStatus,
    certificate_chain_status: SignatureCheckStatus,
}

fn cms_verification(
    dictionary: &Dictionary,
    byte_range: &ByteRangeVerification,
    input: &[u8],
    trust_anchors: &TrustAnchors,
    diagnostics: &mut Vec<SignatureDiagnostic>,
) -> CmsVerificationReport {
    let Some(contents) = dictionary.get(b"Contents").ok().and_then(pdf_string_bytes) else {
        return CmsVerificationReport {
            cms_status: signature_check(
                SignatureCheckState::Failed,
                "signature Contents is missing",
            ),
            digest_status: signature_check(
                SignatureCheckState::Indeterminate,
                "signed byte digest cannot be checked without CMS",
            ),
            signature_status: signature_check(
                SignatureCheckState::Indeterminate,
                "signer signature cannot be checked without CMS",
            ),
            certificate_chain_status: signature_check(
                SignatureCheckState::Indeterminate,
                "certificate chain cannot be checked without CMS",
            ),
        };
    };
    let Some(signed_bytes) = signed_bytes(input, byte_range) else {
        return CmsVerificationReport {
            cms_status: signature_check(
                SignatureCheckState::Indeterminate,
                "CMS parsing skipped because ByteRange is invalid",
            ),
            digest_status: signature_check(
                SignatureCheckState::Failed,
                "signed byte digest cannot be checked because ByteRange is invalid",
            ),
            signature_status: signature_check(
                SignatureCheckState::Failed,
                "signer signature cannot be checked because ByteRange is invalid",
            ),
            certificate_chain_status: signature_check(
                SignatureCheckState::Indeterminate,
                "certificate chain cannot be checked because ByteRange is invalid",
            ),
        };
    };

    let signed_data = match parse_cms_signed_data(contents) {
        Ok(signed_data) => signed_data,
        Err(()) => {
            diagnostics.push(signature_diagnostic(
                "cms_parse_failed",
                "signature Contents is not valid CMS SignedData DER",
            ));
            return CmsVerificationReport {
                cms_status: signature_check(
                    SignatureCheckState::Failed,
                    "signature Contents is not valid CMS SignedData DER",
                ),
                digest_status: signature_check(
                    SignatureCheckState::Indeterminate,
                    "signed byte digest cannot be checked without parsed CMS",
                ),
                signature_status: signature_check(
                    SignatureCheckState::Indeterminate,
                    "signer signature cannot be checked without parsed CMS",
                ),
                certificate_chain_status: signature_check(
                    SignatureCheckState::Indeterminate,
                    "certificate chain cannot be checked without parsed CMS",
                ),
            };
        }
    };

    let certificate_count = signed_data
        .certificates
        .as_ref()
        .map(|certificates| {
            certificates
                .0
                .iter()
                .filter(|choice| matches!(choice, CertificateChoices::Certificate(_)))
                .count()
        })
        .unwrap_or(0);

    CmsVerificationReport {
        cms_status: signature_check(
            SignatureCheckState::Passed,
            format!(
                "CMS SignedData parsed with {} signer(s) and {} X.509 certificate(s)",
                signed_data.signer_infos.0.len(),
                certificate_count
            ),
        ),
        digest_status: cms_digest_verification(&signed_data, &signed_bytes),
        signature_status: cms_signature_verification(&signed_data, &signed_bytes),
        certificate_chain_status: cms_certificate_chain_status(&signed_data, trust_anchors),
    }
}

fn parse_cms_signed_data(contents: &[u8]) -> Result<SignedData, ()> {
    let contents = der_slice_without_padding(contents).unwrap_or(contents);
    if let Ok(content_info) = ContentInfo::from_der(contents)
        && content_info.content_type == const_oid::db::rfc5911::ID_SIGNED_DATA
    {
        return content_info
            .content
            .decode_as::<SignedData>()
            .map_err(|_| ());
    }

    SignedData::from_der(contents).map_err(|_| ())
}

fn der_slice_without_padding(contents: &[u8]) -> Option<&[u8]> {
    if contents.len() < 2 || contents[0] != 0x30 {
        return None;
    }
    let first_len = contents[1];
    let (header_len, value_len) = if first_len & 0x80 == 0 {
        (2usize, first_len as usize)
    } else {
        let len_bytes = usize::from(first_len & 0x7f);
        if len_bytes == 0 || len_bytes > std::mem::size_of::<usize>() {
            return None;
        }
        let len_start = 2usize;
        let len_end = len_start.checked_add(len_bytes)?;
        let mut value_len = 0usize;
        for byte in contents.get(len_start..len_end)? {
            value_len = value_len
                .checked_mul(256)?
                .checked_add(usize::from(*byte))?;
        }
        (len_end, value_len)
    };
    let total_len = header_len.checked_add(value_len)?;
    (total_len <= contents.len()).then_some(&contents[..total_len])
}

fn signed_bytes(input: &[u8], byte_range: &ByteRangeVerification) -> Option<Vec<u8>> {
    if !byte_range.in_bounds || !byte_range.ordered_non_overlapping {
        return None;
    }
    let [first_start, first_len, second_start, second_len] = byte_range.values?;
    let first_start = usize::try_from(first_start).ok()?;
    let first_len = usize::try_from(first_len).ok()?;
    let second_start = usize::try_from(second_start).ok()?;
    let second_len = usize::try_from(second_len).ok()?;
    let first_end = first_start.checked_add(first_len)?;
    let second_end = second_start.checked_add(second_len)?;
    let mut bytes = Vec::with_capacity(first_len.checked_add(second_len)?);
    bytes.extend_from_slice(input.get(first_start..first_end)?);
    bytes.extend_from_slice(input.get(second_start..second_end)?);

    Some(bytes)
}

fn cms_digest_verification(signed_data: &SignedData, signed_bytes: &[u8]) -> SignatureCheckStatus {
    let Some(signer_info) = signed_data.signer_infos.0.iter().next() else {
        return signature_check(
            SignatureCheckState::Failed,
            "CMS SignedData contains no signerInfo entries",
        );
    };
    let Some(signed_attrs) = signer_info.signed_attrs.as_ref() else {
        return signature_check(
            SignatureCheckState::Unsupported,
            "CMS signerInfo has no signed attributes; detached digest verification for this form is not implemented",
        );
    };
    let Some(message_digest) = signed_attrs.iter().find_map(message_digest_attribute) else {
        return signature_check(
            SignatureCheckState::Failed,
            "CMS signerInfo signed attributes are missing messageDigest",
        );
    };

    let Some(computed_digest) = digest_for_algorithm(&signer_info.digest_alg.oid, signed_bytes)
    else {
        return signature_check(
            SignatureCheckState::Unsupported,
            format!(
                "unsupported CMS digest algorithm {}",
                signer_info.digest_alg.oid
            ),
        );
    };

    if computed_digest == message_digest {
        signature_check(
            SignatureCheckState::Passed,
            "CMS messageDigest matches signed bytes",
        )
    } else {
        signature_check(
            SignatureCheckState::Failed,
            "CMS messageDigest does not match signed bytes",
        )
    }
}

fn cms_signature_verification(
    signed_data: &SignedData,
    signed_bytes: &[u8],
) -> SignatureCheckStatus {
    let Some(signer_info) = signed_data.signer_infos.0.iter().next() else {
        return signature_check(
            SignatureCheckState::Failed,
            "CMS SignedData contains no signerInfo entries",
        );
    };
    let Some(certificates) = signed_data.certificates.as_ref() else {
        return signature_check(
            SignatureCheckState::Failed,
            "CMS SignedData contains no embedded certificates",
        );
    };
    let Some(certificate) = signer_certificate(certificates, &signer_info.sid) else {
        return signature_check(
            SignatureCheckState::Failed,
            "CMS signer certificate was not found in embedded certificates",
        );
    };

    let signature_input = if let Some(signed_attrs) = signer_info.signed_attrs.as_ref() {
        match signed_attributes_signature_input(signed_attrs) {
            Some(input) => input,
            None => {
                return signature_check(
                    SignatureCheckState::Indeterminate,
                    "CMS signed attributes could not be re-encoded for signature verification",
                );
            }
        }
    } else {
        signed_bytes.to_vec()
    };

    verify_signer_signature(
        &certificate.tbs_certificate.subject_public_key_info,
        &signer_info.signature_algorithm.oid,
        &signer_info.digest_alg.oid,
        &signature_input,
        signer_info.signature.as_bytes(),
    )
}
