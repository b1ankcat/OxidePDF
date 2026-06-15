use crate::{
    Artifact, TextArtifact, TextExtractionDiagnostic, TextExtractionDiagnosticCode,
    workflow::execution::artifact_size,
};

#[test]
fn text_artifact_size_includes_diagnostics() {
    let text = TextArtifact {
        text: "abc".to_owned(),
        diagnostics: vec![TextExtractionDiagnostic {
            page: 1,
            code: TextExtractionDiagnosticCode::NoTextLayer,
            message: "no text layer".to_owned(),
        }],
    };

    // Size must account for the diagnostics, not just the text body, so the
    // resource-limit check cannot be bypassed with diagnostics-heavy output.
    assert!(artifact_size(&Artifact::Text(text)) > "abc".len());
}
