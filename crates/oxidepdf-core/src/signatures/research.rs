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
