use super::Artifact;
use super::execution::enforce_artifact_output_bytes;
use super::types::{OperatorSpec, ResourceLimits, TaskSpec};
use crate::OxideError;
use crate::operators::{
    run_pdf_compare, run_pdf_edit, run_pdf_inspect, run_pdf_security, run_pdf_sign,
};

/// Operator implementation boundary used by the executor.
///
/// `run` takes `&self` and the trait requires `Sync` so the executor can invoke
/// it concurrently across the tasks of a single dependency layer.
pub trait OperatorRunner: Sync {
    /// Runs a task against resolved input artifacts.
    fn run(&self, task: &TaskSpec, inputs: &[Artifact]) -> Result<Artifact, OxideError>;
}

/// Operator runner for object-level PDF page editing.
#[derive(Debug, Clone, Default)]
pub struct PdfOperatorRunner {
    limits: ResourceLimits,
}

impl PdfOperatorRunner {
    /// Creates a runner using explicit workflow resource limits.
    pub fn with_limits(limits: ResourceLimits) -> Self {
        Self { limits }
    }
}

impl OperatorRunner for PdfOperatorRunner {
    fn run(&self, task: &TaskSpec, inputs: &[Artifact]) -> Result<Artifact, OxideError> {
        let artifact = match &task.op {
            OperatorSpec::PdfEdit(options) => run_pdf_edit(options, inputs, &self.limits),
            OperatorSpec::PdfInspect(options) => run_pdf_inspect(options, inputs, &self.limits),
            OperatorSpec::PdfSecurity(options) => run_pdf_security(options, inputs, &self.limits),
            OperatorSpec::PdfCompare(options) => run_pdf_compare(options, inputs, &self.limits),
            OperatorSpec::PdfSign(options) => run_pdf_sign(options, inputs, &self.limits),
        }?;
        enforce_artifact_output_bytes(&artifact, &self.limits)?;
        Ok(artifact)
    }
}
