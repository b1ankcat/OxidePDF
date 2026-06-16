use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use oxidepdf_core::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Write,
    sync::{Arc, Mutex},
};
use tempfile::NamedTempFile;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

mod schema;
pub use schema::{FamilySchema, OpMeta};

const INDEX_HTML: &str = include_str!("../static/index.html");
const STYLE_CSS: &str = include_str!("../static/style.css");
const FORM_JS: &str = include_str!("../static/form.js");
const APP_JS: &str = include_str!("../static/app.js");

/// How an uploaded/produced artifact should be reconstructed and served.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Pdf,
    Image,
    Svg,
    Text,
    Bytes,
}

impl Kind {
    fn content_type(self) -> &'static str {
        match self {
            Kind::Pdf => "application/pdf",
            Kind::Image => "image/png",
            Kind::Svg => "image/svg+xml",
            Kind::Text => "text/plain; charset=utf-8",
            Kind::Bytes => "application/octet-stream",
        }
    }

    /// Build the matching core artifact from raw bytes. Only the kinds an
    /// upload can produce are constructible here; result-only kinds (Text,
    /// Bytes) are never used to rebuild an input and map to Pdf bytes if asked.
    fn artifact(self, bytes: Vec<u8>) -> Result<Artifact, AppError> {
        match self {
            Kind::Image => Artifact::image(bytes),
            Kind::Svg => Artifact::svg(bytes),
            _ => Artifact::pdf(bytes),
        }
        .map_err(internal)
    }
}

/// Classify an upload by filename extension. Unknown extensions default to PDF
/// (PDF construction validates the magic bytes, so non-PDF junk is rejected
/// downstream rather than silently mistyped).
fn kind_from_filename(name: &str) -> Kind {
    match name
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png" | "jpg" | "jpeg" | "webp") => Kind::Image,
        Some("svg") => Kind::Svg,
        _ => Kind::Pdf,
    }
}

struct StoredArtifact {
    file: NamedTempFile,
    kind: Kind,
}

#[derive(Clone)]
pub struct AppState {
    artifacts: Arc<Mutex<HashMap<String, StoredArtifact>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            artifacts: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

struct AppError(StatusCode, String);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

fn bad_req(msg: impl ToString) -> AppError {
    AppError(StatusCode::BAD_REQUEST, msg.to_string())
}
fn not_found() -> AppError {
    AppError(StatusCode::NOT_FOUND, "not found".into())
}
fn internal(e: impl ToString) -> AppError {
    AppError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn artifact_kind(a: &Artifact) -> Kind {
    match a {
        Artifact::Pdf(_) | Artifact::PdfObject(_) => Kind::Pdf,
        Artifact::Image(_) => Kind::Image,
        Artifact::Svg(_) => Kind::Svg,
        Artifact::Text(_) => Kind::Text,
        Artifact::Bytes(_) => Kind::Bytes,
    }
}

async fn store_artifact(state: &AppState, artifact: Artifact) -> Result<String, AppError> {
    let kind = artifact_kind(&artifact);
    let bytes = artifact.output_bytes().map_err(internal)?;
    let mut tmp = NamedTempFile::new().map_err(internal)?;
    tmp.write_all(&bytes).map_err(internal)?;
    let id = Uuid::new_v4().to_string();
    state
        .artifacts
        .lock()
        .unwrap()
        .insert(id.clone(), StoredArtifact { file: tmp, kind });
    Ok(id)
}

async fn build_store(
    state: &AppState,
    ids: &[String],
) -> Result<(Vec<ArtifactRef>, ArtifactStore), AppError> {
    let mut store = ArtifactStore::new();
    let mut refs = Vec::with_capacity(ids.len());
    let guard = state.artifacts.lock().unwrap();
    for (i, id) in ids.iter().enumerate() {
        let stored = guard.get(id).ok_or_else(not_found)?;
        let bytes = std::fs::read(stored.file.path()).map_err(internal)?;
        let r = ArtifactRef::new(format!("i{i}"));
        store.insert(r.clone(), stored.kind.artifact(bytes)?);
        refs.push(r);
    }
    Ok((refs, store))
}

// from_ref is the last task's ID — the result store keys artifacts by task ID, not OutputSpec.id
async fn run_workflow(
    wf: Workflow,
    store: ArtifactStore,
    from_ref: ArtifactRef,
) -> Result<Artifact, AppError> {
    let runner = PdfOperatorRunner::with_limits(wf.limits.clone());
    let mut result = execute_workflow(&wf, store, runner)
        .await
        .map_err(internal)?;
    result
        .store
        .remove(&from_ref)
        .ok_or_else(|| internal("output artifact missing"))
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
async fn api_schema() -> Json<Vec<FamilySchema>> {
    Json(schema::schema())
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
    while let Some(field) = mp.next_field().await.map_err(bad_req)? {
        let filename = field.file_name().unwrap_or("file").to_string();
        let kind = kind_from_filename(&filename);
        let bytes = field.bytes().await.map_err(bad_req)?;
        let mut tmp = NamedTempFile::new().map_err(internal)?;
        tmp.write_all(&bytes).map_err(internal)?;
        let id = Uuid::new_v4().to_string();
        state
            .artifacts
            .lock()
            .unwrap()
            .insert(id.clone(), StoredArtifact { file: tmp, kind });
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
    let op_spec = schema::parse_op(&req.family, &req.op, &req.options_json).map_err(bad_req)?;
    let (in_refs, store) = build_store(&state, &req.artifact_ids).await?;
    let task_ref = ArtifactRef::new("t0");
    let wf = Workflow {
        version: WorkflowVersion::V1,
        inputs: in_refs
            .iter()
            .enumerate()
            .map(|(i, r)| InputSpec {
                id: r.clone(),
                path: format!("i{i}").into(),
            })
            .collect(),
        tasks: vec![TaskSpec {
            id: TaskId::new("t0"),
            op: op_spec,
            inputs: in_refs,
        }],
        outputs: vec![OutputSpec {
            id: ArtifactRef::new("out"),
            from: task_ref.clone(),
            path: "out".into(),
        }],
        limits: ResourceLimits::default(),
        metadata: WorkflowMetadata::default(),
    };
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
fn load_file_bytes(
    state: &AppState,
    tasks: &[WfTask],
) -> Result<Vec<(String, Kind, Vec<u8>)>, AppError> {
    let guard = state.artifacts.lock().unwrap();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for task in tasks {
        for input in &task.inputs {
            if let WfInput::File { id } = input
                && seen.insert(id.clone())
            {
                let stored = guard.get(id).ok_or_else(not_found)?;
                let bytes = std::fs::read(stored.file.path()).map_err(internal)?;
                out.push((id.clone(), stored.kind, bytes));
            }
        }
    }
    Ok(out)
}

async fn api_execute_workflow(
    State(state): State<AppState>,
    Json(req): Json<WorkflowReq>,
) -> Result<impl IntoResponse, AppError> {
    if req.tasks.is_empty() {
        return Err(bad_req("tasks is empty"));
    }

    // Pre-populate the store with every referenced uploaded file, keyed by a
    // sanitized ref derived from the file id, and declare each as a workflow input.
    let mut store = ArtifactStore::new();
    let mut inputs_specs = Vec::new();
    let file_bytes = load_file_bytes(&state, &req.tasks)?;
    for (id, kind, bytes) in file_bytes {
        let r = ArtifactRef::new(format!("f_{id}"));
        store.insert(r.clone(), kind.artifact(bytes)?);
        inputs_specs.push(InputSpec {
            id: r,
            path: format!("f_{id}").into(),
        });
    }

    let n = req.tasks.len();
    let mut task_specs = Vec::with_capacity(n);

    for (ti, task) in req.tasks.iter().enumerate() {
        let op_spec =
            schema::parse_op(&task.family, &task.op, &task.options_json).map_err(bad_req)?;
        if task.inputs.is_empty() {
            return Err(bad_req(format!("task {ti} has no inputs")));
        }
        let mut task_inputs = Vec::with_capacity(task.inputs.len());
        for input in &task.inputs {
            let r = match input {
                WfInput::File { id } => ArtifactRef::new(format!("f_{id}")),
                WfInput::Step { index } => {
                    if *index >= ti {
                        return Err(bad_req(format!(
                            "task {ti} references step {index} which is not earlier"
                        )));
                    }
                    ArtifactRef::new(format!("t{index}"))
                }
            };
            task_inputs.push(r);
        }
        task_specs.push(TaskSpec {
            id: TaskId::new(format!("t{ti}")),
            op: op_spec,
            inputs: task_inputs,
        });
    }

    let last_id = ArtifactRef::new(format!("t{}", n - 1));
    let wf = Workflow {
        version: WorkflowVersion::V1,
        inputs: inputs_specs,
        tasks: task_specs,
        outputs: vec![OutputSpec {
            id: ArtifactRef::new("out"),
            from: last_id.clone(),
            path: "out".into(),
        }],
        limits: ResourceLimits::default(),
        metadata: WorkflowMetadata::default(),
    };
    let artifact = run_workflow(wf, store, last_id).await?;
    let result_id = store_artifact(&state, artifact).await?;
    Ok(Json(serde_json::json!({ "result_id": result_id })))
}

// GET /api/file/:id
async fn api_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let guard = state.artifacts.lock().unwrap();
    let stored = guard.get(&id).ok_or_else(not_found)?;
    let bytes = std::fs::read(stored.file.path()).map_err(internal)?;
    let ct = stored.kind.content_type();
    drop(guard);
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, ct.parse().unwrap());
    Ok((headers, Body::from(bytes)).into_response())
}

// DELETE /api/file/:id
async fn api_delete_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state
        .artifacts
        .lock()
        .unwrap()
        .remove(&id)
        .ok_or_else(not_found)?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    // Match the core ResourceLimits input ceiling (512 MiB) so uploads aren't
    // capped by axum's 2 MiB default before the handler runs.
    let body_limit = 512 * 1024 * 1024;
    Router::new()
        .route("/", get(index))
        .route("/static/{file}", get(static_asset))
        .route("/api/schema", get(api_schema))
        .route("/api/upload", post(api_upload))
        .route("/api/execute/single", post(api_execute_single))
        .route("/api/execute/workflow", post(api_execute_workflow))
        .route("/api/file/{id}", get(api_file).delete(api_delete_file))
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(cors)
        .with_state(state)
}
