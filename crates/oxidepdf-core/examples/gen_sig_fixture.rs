//! Temporary fixture generator for tests/fixtures/signature-placeholder.pdf.
//!
//! Produces a minimal but structurally valid single-page PDF whose signature
//! dictionary carries the exact byte markers the research scanner looks for:
//! `/Type /Sig`, `/SubFilter /adbe.pkcs7.detached`, and `/ByteRange [0 64 192 64]`.
//! The byte ranges are chosen so the scanner reports gap_len = covered_len = 128
//! and the file is large enough (>= 256 bytes) for the ranges to be in-bounds.

use std::fmt::Write as _;

fn main() {
    // Build the body objects, tracking byte offsets for a correct xref table.
    let header = "%PDF-1.7\n%\u{00b5}\u{00b5}\u{00b5}\u{00b5}\n";
    let objects = [
        // 1: Catalog
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R] /SigFlags 3 >> >>".to_string(),
        // 2: Pages
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        // 3: Page
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R] >>".to_string(),
        // 4: Signature widget annotation
        "<< /Type /Annot /Subtype /Widget /FT /Sig /T (Approval) /Rect [0 0 0 0] /P 3 0 R /V 5 0 R >>"
            .to_string(),
        // 5: Signature dictionary with the exact markers the scanner parses.
        "<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached \
/ByteRange [0 64 192 64] /Contents <00> >>"
            .to_string(),
    ];

    let mut pdf = String::new();
    pdf.push_str(header);

    let mut offsets = Vec::with_capacity(objects.len());
    for (index, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        let _ = write!(pdf, "{} 0 obj\n{}\nendobj\n", index + 1, body);
    }

    let xref_offset = pdf.len();
    let count = objects.len() + 1; // including the free object 0
    let _ = write!(pdf, "xref\n0 {count}\n");
    pdf.push_str("0000000000 65535 f \n");
    for offset in &offsets {
        let _ = writeln!(pdf, "{offset:010} 00000 n ");
    }
    let _ = write!(
        pdf,
        "trailer\n<< /Size {count} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
    );

    let bytes = pdf.into_bytes();
    std::fs::write("tests/fixtures/signature-placeholder.pdf", &bytes).unwrap();
    println!(
        "wrote tests/fixtures/signature-placeholder.pdf ({} bytes)",
        bytes.len()
    );
}
