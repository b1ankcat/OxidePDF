use super::*;

pub(crate) fn run_compare(
    command: PdfCompareCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let (left, right, output, force, operator) = match command {
        PdfCompareCommand::Report(args) => (
            args.left,
            args.right,
            args.output,
            args.force,
            PdfCompareOptions::Report(CompareOptions::default()),
        ),
        PdfCompareCommand::VisualDiff(args) => (
            args.left,
            args.right,
            args.output,
            args.force,
            PdfCompareOptions::VisualDiff(VisualDiffOptions {
                page: args.page,
                scale: args.scale,
            }),
        ),
    };
    let workflow = Workflow {
        version: WorkflowVersion::V1,
        inputs: vec![
            oxidepdf_core::InputSpec {
                id: ArtifactRef::new("left"),
                path: left,
            },
            oxidepdf_core::InputSpec {
                id: ArtifactRef::new("right"),
                path: right,
            },
        ],
        tasks: vec![TaskSpec {
            id: TaskId::new("compare"),
            op: OperatorSpec::PdfCompare(operator),
            inputs: vec![ArtifactRef::new("left"), ArtifactRef::new("right")],
        }],
        outputs: vec![oxidepdf_core::OutputSpec {
            id: ArtifactRef::new("output"),
            from: ArtifactRef::new("compare"),
            path: output,
        }],
        limits: Default::default(),
        metadata: WorkflowMetadata::default(),
    };

    execute_and_write_workflow(workflow, stdin, force, stdout)
}
