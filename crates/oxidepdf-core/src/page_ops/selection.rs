use crate::OxideError;
use std::collections::BTreeSet;

pub(crate) fn parse_page_range(pages: &str, page_count: u32) -> Result<Vec<u32>, OxideError> {
    if pages.trim().is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "page range must not be empty".to_owned(),
        });
    }

    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    for part in pages.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(OxideError::InvalidInput {
                reason: "page range contains an empty item".to_owned(),
            });
        }

        if let Some((start, end)) = part.split_once('-') {
            let start = parse_page_number(start.trim(), page_count)?;
            let end = parse_page_number(end.trim(), page_count)?;
            if start > end {
                return Err(OxideError::InvalidInput {
                    reason: format!("page range '{part}' must be ascending"),
                });
            }
            for page in start..=end {
                push_unique(&mut selected, &mut seen, page)?;
            }
        } else {
            push_unique(
                &mut selected,
                &mut seen,
                parse_page_number(part, page_count)?,
            )?;
        }
    }

    Ok(selected)
}

/// Appends `page` to `selected`, rejecting duplicates as they are seen so a
/// crafted range like `1-N,1-N` cannot allocate more than `page_count` entries.
fn push_unique(
    selected: &mut Vec<u32>,
    seen: &mut BTreeSet<u32>,
    page: u32,
) -> Result<(), OxideError> {
    if !seen.insert(page) {
        return Err(OxideError::InvalidInput {
            reason: "page range must not contain duplicate pages".to_owned(),
        });
    }
    selected.push(page);
    Ok(())
}

fn parse_page_number(value: &str, page_count: u32) -> Result<u32, OxideError> {
    let page = value.parse::<u32>().map_err(|_| OxideError::InvalidInput {
        reason: format!("invalid page number '{value}'"),
    })?;
    if page == 0 || page > page_count {
        return Err(OxideError::InvalidInput {
            reason: format!("page {page} is out of range 1-{page_count}"),
        });
    }

    Ok(page)
}

pub(super) fn selected_or_all_pages(
    pages: Option<&str>,
    page_count: u32,
) -> Result<Vec<u32>, OxideError> {
    match pages {
        Some(pages) => parse_page_range(pages, page_count),
        None => Ok((1..=page_count).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_page_range_accepts_ranges_and_single_pages() {
        assert_eq!(parse_page_range("1,3-5", 6).unwrap(), vec![1, 3, 4, 5]);
    }

    #[test]
    fn parse_page_range_rejects_empty_ranges() {
        let error = parse_page_range("", 3).unwrap_err();
        assert!(matches!(error, OxideError::InvalidInput { .. }));
    }

    #[test]
    fn parse_page_range_rejects_descending_ranges() {
        let error = parse_page_range("3-1", 3).unwrap_err();
        assert!(matches!(error, OxideError::InvalidInput { .. }));
    }

    #[test]
    fn parse_page_range_rejects_duplicate_pages() {
        let error = parse_page_range("1,1", 3).unwrap_err();
        assert!(matches!(error, OxideError::InvalidInput { .. }));
    }

    #[test]
    fn parse_page_range_rejects_overlapping_ranges_before_full_expansion() {
        // Overlapping ranges must be rejected on the first duplicate rather than
        // expanding both ranges in full first.
        let error = parse_page_range("1-3,2-3", 3).unwrap_err();
        assert!(matches!(error, OxideError::InvalidInput { .. }));
    }

    #[test]
    fn selected_or_all_pages_defaults_to_every_page() {
        assert_eq!(selected_or_all_pages(None, 3).unwrap(), vec![1, 2, 3]);
    }
}
