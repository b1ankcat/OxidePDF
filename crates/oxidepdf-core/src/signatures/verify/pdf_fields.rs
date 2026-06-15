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

fn pdf_string_bytes(object: &lopdf::Object) -> Option<&[u8]> {
    object.as_str().ok()
}
