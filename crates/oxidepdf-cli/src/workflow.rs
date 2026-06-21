use super::*;
use serde_json::json;
use std::time::Instant;

pub(crate) async fn run_workflow(
    args: RunArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    if args.metrics_output.as_deref().is_some_and(is_stdio) {
        return Err(CliError::Workflow(
            "metrics output cannot be '-'".to_owned(),
        ));
    }
    if let Some(path) = &args.metrics_output {
        check_metrics_output_path(path, args.force)?;
    }

    let workflow_from_stdin = is_stdio(&args.workflow);
    let workflow_bytes = read_path_or_stdin(&args.workflow, stdin, &ResourceLimits::default())?;
    let workflow = parse_workflow(&workflow_bytes, &args.workflow)?;

    let stdin_inputs = workflow
        .inputs
        .iter()
        .filter(|input| is_stdio(&input.path))
        .count();
    if workflow_from_stdin && stdin_inputs > 0 {
        return Err(CliError::Workflow(
            "workflow read from stdin cannot also declare a stdin ('-') input".to_owned(),
        ));
    }
    if stdin_inputs > 1 {
        return Err(CliError::Workflow(
            "workflow cannot read more than one input from stdin ('-')".to_owned(),
        ));
    }

    let (store, input_bytes) = load_inputs(&workflow, stdin)?;
    let runner = PdfOperatorRunner::with_limits(workflow.limits.clone());
    let started_at = Instant::now();
    let result = execute_workflow(&workflow, store, runner)
        .await
        .map_err(CliError::Core)?;
    let output_bytes = write_outputs_with_stats(&workflow, &result.store, args.force, stdout)?;
    let elapsed_ms = started_at.elapsed().as_millis() as u64;

    if let Some(path) = args.metrics_output {
        let peak_rss_bytes = linux_peak_rss_bytes()?;
        let metrics = json!({
            "version": 1,
            "elapsed_ms": elapsed_ms,
            "input_bytes": input_bytes,
            "output_bytes": output_bytes,
            "task_count": workflow.tasks.len() as u64,
            "peak_rss_bytes": peak_rss_bytes,
            "peak_rss_source": peak_rss_bytes.map(|_| "/proc/self/status:VmHWM"),
        });
        write_metrics_output(&path, &metrics, args.force)?;
    }

    Ok(())
}

fn write_metrics_output(
    path: &Path,
    metrics: &serde_json::Value,
    force: bool,
) -> Result<(), CliError> {
    check_metrics_output_path(path, force)?;
    let bytes =
        serde_json::to_vec_pretty(metrics).map_err(|_| CliError::Core(OxideError::Internal))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(CliError::Io)?;
    file.write_all(&bytes).map_err(CliError::Io)?;
    persist_output_file(file, path, force)
}

fn check_metrics_output_path(path: &Path, force: bool) -> Result<(), CliError> {
    if path.exists() && !force {
        return Err(CliError::Workflow(format!(
            "metrics output file already exists: {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_peak_rss_bytes() -> Result<Option<u64>, CliError> {
    let status = fs::read_to_string("/proc/self/status")
        .map_err(|_| CliError::Core(OxideError::ArtifactStorage))?;
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("VmHWM:") else {
            continue;
        };
        let mut parts = rest.split_whitespace();
        let value = parts
            .next()
            .ok_or(CliError::Core(OxideError::ArtifactStorage))?
            .parse::<u64>()
            .map_err(|_| CliError::Core(OxideError::ArtifactStorage))?;
        let unit = parts
            .next()
            .ok_or(CliError::Core(OxideError::ArtifactStorage))?;
        if unit != "kB" {
            return Err(CliError::Core(OxideError::ArtifactStorage));
        }
        return value
            .checked_mul(1024)
            .ok_or(CliError::Core(OxideError::ArtifactStorage))
            .map(Some);
    }
    Ok(None) // VmHWM absent from /proc/self/status (some container runtimes)
}

#[cfg(not(target_os = "linux"))]
fn linux_peak_rss_bytes() -> Result<Option<u64>, CliError> {
    Ok(None)
}
