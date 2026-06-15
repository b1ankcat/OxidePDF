use super::Artifact;
use super::execution::enforce_artifact_output_bytes;
use super::types::{OperatorSpec, ResourceLimits, TaskSpec};
use crate::OxideError;
use crate::operators::{
    run_pdf_compare, run_pdf_edit, run_pdf_inspect, run_pdf_security, run_pdf_sign,
};
use std::future::Future;
use std::pin::Pin;

pub type OperatorFuture = Pin<Box<dyn Future<Output = Result<Artifact, OxideError>> + Send>>;

/// Operator implementation boundary used by the executor.
pub trait OperatorRunner: Sync + Send + 'static {
    /// Runs a task against resolved input artifacts.
    fn run(&self, task: TaskSpec, inputs: Vec<Artifact>) -> OperatorFuture;
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
    fn run(&self, task: TaskSpec, inputs: Vec<Artifact>) -> OperatorFuture {
        let limits = self.limits.clone();
        Box::pin(async move {
            let artifact = match &task.op {
                OperatorSpec::PdfEdit(options) => run_pdf_edit(options, &inputs, &limits),
                OperatorSpec::PdfInspect(options) => run_pdf_inspect(options, &inputs, &limits),
                OperatorSpec::PdfSecurity(options) => run_pdf_security(options, &inputs, &limits),
                OperatorSpec::PdfCompare(options) => run_pdf_compare(options, &inputs, &limits),
                OperatorSpec::PdfSign(options) => run_pdf_sign(options, &inputs, &limits),
            }?;
            enforce_artifact_output_bytes(&artifact, &limits)?;
            Ok(artifact)
        })
    }
}
