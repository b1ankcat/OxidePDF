
// from_ref is the last task's ID — the result store keys artifacts by task ID, not OutputSpec.id
async fn run_workflow(
    wf: Workflow,
    store: ArtifactStore,
    from_ref: ArtifactRef,
) -> Result<Artifact, AppError> {
    let runner = PdfOperatorRunner::with_limits(wf.limits.clone());
    let mut result = execute_workflow(&wf, store, runner)
        .await
        .map_err(core_error)?;
    result
        .store
        .remove(&from_ref)
        .ok_or_else(|| internal("output artifact missing"))
}

/// Assemble a `Workflow` from input refs and task specs, with the single output
/// taken from `last`. Shared by the single-op and workflow execute paths.
fn web_resource_limits(max_bytes: u64) -> ResourceLimits {
    ResourceLimits {
        max_input_bytes: Some(max_bytes),
        max_total_input_bytes: Some(max_bytes),
        max_output_bytes: Some(max_bytes),
        timeout_ms: Some(WEB_WORKFLOW_TIMEOUT_MS),
        ..ResourceLimits::default()
    }
}

fn build_workflow(
    inputs: Vec<InputSpec>,
    tasks: Vec<TaskSpec>,
    last: ArtifactRef,
    max_bytes: u64,
) -> Workflow {
    Workflow {
        version: WorkflowVersion::V1,
        inputs,
        tasks,
        outputs: vec![OutputSpec {
            id: ArtifactRef::new("out"),
            from: last,
            path: "out".into(),
        }],
        limits: web_resource_limits(max_bytes),
        metadata: WorkflowMetadata::default(),
    }
}

fn validate_single_request(req: &SingleReq) -> Result<(), AppError> {
    if req.artifact_ids.is_empty() {
        return Err(bad_req("artifact_ids is empty"));
    }
    if req.artifact_ids.len() > MAX_SINGLE_INPUTS {
        return Err(bad_req(format!(
            "too many input artifacts: max {MAX_SINGLE_INPUTS}"
        )));
    }
    Ok(())
}

fn validate_workflow_request(req: &WorkflowReq) -> Result<(), AppError> {
    if req.tasks.is_empty() {
        return Err(bad_req("tasks is empty"));
    }
    if req.tasks.len() > MAX_WORKFLOW_TASKS {
        return Err(bad_req(format!(
            "too many workflow tasks: max {MAX_WORKFLOW_TASKS}"
        )));
    }
    for (index, task) in req.tasks.iter().enumerate() {
        if task.inputs.is_empty() {
            return Err(bad_req(format!("task {index} has no inputs")));
        }
        if task.inputs.len() > MAX_TASK_INPUTS {
            return Err(bad_req(format!(
                "task {index} has too many inputs: max {MAX_TASK_INPUTS}"
            )));
        }
    }
    Ok(())
}

// GET /
async fn index() -> Response {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        INDEX_HTML,
    )
        .into_response()
}

// GET /static/{file}
async fn static_asset(Path(file): Path<String>) -> Response {
    let (ct, body) = match file.as_str() {
        "style.css" => ("text/css; charset=utf-8", STYLE_CSS),
        "form.js" => ("text/javascript; charset=utf-8", FORM_JS),
        "app.js" => ("text/javascript; charset=utf-8", APP_JS),
        _ => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };
    ([(header::CONTENT_TYPE, ct)], body).into_response()
}

// GET /api/schema
async fn api_schema() -> Result<Json<Vec<FamilySchema>>, AppError> {
    let schema = schema::schema().map_err(internal)?;
    Ok(Json(schema))
}

// POST /api/upload
async fn api_upload(
    State(state): State<AppState>,
    mut mp: Multipart,
) -> Result<impl IntoResponse, AppError> {
    #[derive(Serialize)]
    struct FileRef {
        id: String,
        filename: String,
    }

    let mut files = Vec::new();
    while let Some(mut field) = mp.next_field().await.map_err(bad_req)? {
        if files.len() >= MAX_UPLOAD_FILES {
            return Err(bad_req(format!("too many files: max {MAX_UPLOAD_FILES}")));
        }
        let filename = field
            .file_name()
            .ok_or_else(|| bad_req("uploaded file is missing a filename"))?
            .to_string();
        let kind = kind_from_filename(&filename)?;
        let content_type = upload_content_type(&filename, kind);
        let mut tmp = blocking_io(upload_temp_file).await?;
        let mut size = 0u64;
        while let Some(chunk) = field.chunk().await.map_err(bad_req)? {
            size = size
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| bad_req("file too large"))?;
            let max_upload_bytes = state.max_upload_bytes();
            if size > max_upload_bytes {
                return Err(bad_req(format!(
                    "file too large: max {max_upload_bytes} bytes"
                )));
            }
            tmp = write_temp_chunk(tmp, chunk).await?;
        }
        let id = state.store_file(tmp, kind, content_type, size)?;
        files.push(FileRef { id, filename });
    }
    Ok(Json(serde_json::json!({ "files": files })))
}

// POST /api/execute/single
#[derive(Deserialize)]
struct SingleReq {
    artifact_ids: Vec<String>,
    family: String,
    op: String,
    options_json: String,
}

async fn api_execute_single(
    State(state): State<AppState>,
    Json(req): Json<SingleReq>,
) -> Result<impl IntoResponse, AppError> {
    validate_single_request(&req)?;
    let op_spec = schema::parse_op(&req.family, &req.op, &req.options_json).map_err(bad_req)?;
    let (in_refs, store) = build_store(&state, &req.artifact_ids).await?;
    let task_ref = ArtifactRef::new("t0");
    let inputs = in_refs
        .iter()
        .enumerate()
        .map(|(i, r)| InputSpec {
            id: r.clone(),
            path: format!("i{i}").into(),
        })
        .collect();
    let tasks = vec![TaskSpec {
        id: TaskId::new("t0"),
        op: op_spec,
        inputs: in_refs,
    }];
    let wf = build_workflow(inputs, tasks, task_ref.clone(), state.max_upload_bytes());
    let artifact = run_workflow(wf, store, task_ref).await?;
    let result_id = store_artifact(&state, artifact).await?;
    Ok(Json(serde_json::json!({ "result_id": result_id })))
}

// POST /api/execute/workflow
// A task input is either an uploaded file or the output of an earlier task.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WfInput {
    File { id: String },
    Step { index: usize },
}

#[derive(Deserialize)]
struct WfTask {
    family: String,
    op: String,
    options_json: String,
    inputs: Vec<WfInput>,
}

#[derive(Deserialize)]
struct WorkflowReq {
    tasks: Vec<WfTask>,
}

// Load (id, kind, bytes) for every distinct file id referenced by any task input.
async fn load_file_bytes(
    state: &AppState,
    tasks: &[WfTask],
) -> Result<Vec<(String, Kind, Vec<u8>)>, AppError> {
    // Collect distinct ids in first-seen order, then read each off the runtime.
    let mut seen = std::collections::HashSet::new();
    let mut ids = Vec::new();
    for task in tasks {
        for input in &task.inputs {
            if let WfInput::File { id } = input
                && seen.insert(id.clone())
            {
                ids.push(id.clone());
            }
        }
    }
    let mut out = Vec::with_capacity(ids.len());
    enforce_total_stored_size(state, ids.iter().map(String::as_str))?;
    for id in ids {
        let (bytes, kind, _) = read_stored(state, &id).await?;
        out.push((id, kind, bytes));
    }
    Ok(out)
}
