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
            covers_whole_input: false,
            gap_len: None,
            covered_len: None,
        };
    };
    let [first_start, first_len, second_start, second_len] = values;
    let input_len = input.len() as u64;
    let research = byte_range_research(first_start, first_len, second_start, second_len, input_len);
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
    // The two signed ranges must span the entire document: the first range must
    // start at byte 0 and the second range must end exactly at end-of-file. Any
    // bytes before the first range or after the second range are unsigned and
    // would let an attacker append or prepend arbitrary content while keeping a
    // valid signature over the original bytes (signature-wrapping attack).
    let second_end = second_start.checked_add(second_len);
    let covers_whole_input =
        first_start == 0 && second_end.is_some_and(|end| end == input_len);
    if research.in_bounds && research.ordered_non_overlapping && !covers_whole_input {
        diagnostics.push(signature_diagnostic(
            "byte_range_not_full_coverage",
            "ByteRange does not cover the whole document; unsigned bytes are present outside the signed ranges",
        ));
    }

    ByteRangeVerification {
        values: Some(values),
        in_bounds: research.in_bounds,
        ordered_non_overlapping: research.ordered_non_overlapping,
        covers_whole_input,
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
    // The unsigned gap between the two signed ranges must contain exactly the
    // hexadecimal Contents string token and nothing else: `<` + 2 hex digits per
    // content byte + `>`. A gap larger than that token leaves room for unsigned
    // bytes inside the document, so we require an exact match rather than `>=`.
    let expected_gap = (byte_len as u64)
        .checked_mul(2)
        .and_then(|hex_len| hex_len.checked_add(2));
    let covered_by_gap = match (byte_range.gap_len, expected_gap) {
        (Some(gap_len), Some(expected_gap)) => gap_len == expected_gap,
        _ => false,
    };
    if !covered_by_gap {
        diagnostics.push(signature_diagnostic(
            "contents_not_covered_by_gap",
            "signature Contents does not exactly fill the unsigned ByteRange gap",
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
