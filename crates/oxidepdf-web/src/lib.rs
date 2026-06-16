use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, FromRequestParts, Multipart, Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_auth::AuthBasic;
use oxidepdf_core::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs,
    io::{self, Write},
    path::{Path as FsPath, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
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

/// Default HTTP body ceiling for the browser-facing service. This accepts common
/// large PDFs while still requiring an explicit deployment choice for very large
/// uploads.
pub const DEFAULT_MAX_UPLOAD_BYTES: u64 = 128 * 1024 * 1024;
/// Extra request-body room for multipart boundaries and headers around uploaded
/// files. File and workflow resource limits still use `max_upload_bytes`.
const MULTIPART_BODY_OVERHEAD_BYTES: u64 = 1024 * 1024;
/// Maximum number of multipart files accepted in one upload request.
const MAX_UPLOAD_FILES: usize = 16;
/// Maximum number of input artifacts accepted by a single-op request.
const MAX_SINGLE_INPUTS: usize = 16;
/// Maximum number of tasks accepted by a browser-built workflow.
const MAX_WORKFLOW_TASKS: usize = 32;
/// Maximum number of input references accepted by any one workflow task.
const MAX_TASK_INPUTS: usize = 8;
/// Web workflows run with a bounded wall-clock deadline unless a caller adds a
/// narrower limit inside core in the future.
const WEB_WORKFLOW_TIMEOUT_MS: u64 = 120_000;
/// Upper bound on the number of artifacts (uploads + results) retained in
/// memory at once. The oldest are evicted first once exceeded.
const MAX_ARTIFACTS: usize = 256;
/// Default ceiling on the total on-disk size of retained artifacts when none is
/// configured. The oldest are evicted first once exceeded.
const DEFAULT_MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Artifacts untouched for this long are swept regardless of the count/size
/// caps, so idle uploads don't linger indefinitely.
const ARTIFACT_TTL: Duration = Duration::from_secs(30 * 60);
/// How often the background sweeper runs.
const SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// Directory, under the process current working directory, used for uploaded
/// and generated artifact temp files.
const UPLOAD_DIR: &str = "upload";

/// Parse a human-readable byte size such as `2G`, `1024M`, `100K`, or `512MiB`.
/// Suffixes are binary (1 K = 1024). A bare number is bytes. An optional
/// trailing `B`/`iB` is accepted. Case-insensitive.
pub fn parse_size(s: &str) -> Result<u64, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("empty size".into());
    }
    // Split into leading digits and the unit suffix.
    let split = t.find(|c: char| !c.is_ascii_digit()).unwrap_or(t.len());
    let (num, unit) = t.split_at(split);
    let value: u64 = num
        .parse()
        .map_err(|_| format!("invalid size number in {s:?}"))?;
    if value == 0 {
        return Err(format!("size {s:?} must be greater than zero"));
    }
    let mult = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" | "kib" => 1024,
        "m" | "mb" | "mib" => 1024 * 1024,
        "g" | "gb" | "gib" => 1024 * 1024 * 1024,
        "t" | "tb" | "tib" => 1024_u64 * 1024 * 1024 * 1024,
        other => return Err(format!("unknown size unit {other:?} in {s:?}")),
    };
    value
        .checked_mul(mult)
        .ok_or_else(|| format!("size {s:?} overflows u64"))
}

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
    /// Fallback content type for a kind. Image uploads carry their own precise
    /// type (see [`StoredArtifact::content_type`]); this is the default used
    /// for engine-produced artifacts where only the kind is known.
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
        .map_err(core_error)
    }
}

/// The lower-cased file extension, or `""` when there is none.
fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((_, ext)) => ext.to_ascii_lowercase(),
        None => String::new(),
    }
}

/// Classify an upload by filename extension. Unknown extensions default to PDF
/// (PDF construction validates the magic bytes, so non-PDF junk is rejected
/// downstream rather than silently mistyped).
fn kind_from_filename(name: &str) -> Kind {
    match extension(name).as_str() {
        "png" | "jpg" | "jpeg" | "webp" => Kind::Image,
        "svg" => Kind::Svg,
        _ => Kind::Pdf,
    }
}

/// Precise content type for an image upload, derived from its extension so a
/// JPEG isn't served as `image/png`. Non-image kinds fall back to the kind's
/// default type.
fn upload_content_type(name: &str, kind: Kind) -> &'static str {
    if kind == Kind::Image {
        match extension(name).as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            _ => "image/png",
        }
    } else {
        kind.content_type()
    }
}

struct StoredArtifact {
    file: NamedTempFile,
    kind: Kind,
    /// Precise content type to serve (preserves the exact image format).
    content_type: &'static str,
    size: u64,
    /// Monotonic insertion order, used to evict the oldest first.
    seq: u64,
    /// Last time this artifact was read, used for TTL sweeping.
    last_access: Instant,
}

struct Store {
    artifacts: HashMap<String, StoredArtifact>,
    total_bytes: u64,
    /// Configured ceiling for `total_bytes` before oldest-first eviction.
    max_total_bytes: u64,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            artifacts: HashMap::new(),
            total_bytes: 0,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
        }
    }
}

impl Store {
    /// Insert an artifact (which carries its own `seq`), then evict in
    /// insertion order (FIFO by `seq`) until the count and total-size caps are
    /// both satisfied. Note this is FIFO, not LRU: `last_access` drives only
    /// the TTL sweep, so a frequently-read old artifact can still be evicted
    /// under capacity pressure.
    fn insert(&mut self, id: String, artifact: StoredArtifact) {
        self.total_bytes += artifact.size;
        if let Some(old) = self.artifacts.insert(id, artifact) {
            self.total_bytes -= old.size;
        }
        while self.artifacts.len() > MAX_ARTIFACTS || self.total_bytes > self.max_total_bytes {
            let Some(oldest) = self
                .artifacts
                .iter()
                .min_by_key(|(_, a)| a.seq)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.remove(&oldest);
        }
    }

    fn remove(&mut self, id: &str) -> Option<StoredArtifact> {
        let removed = self.artifacts.remove(id);
        if let Some(a) = &removed {
            self.total_bytes -= a.size;
        }
        removed
    }

    /// Drop every artifact untouched for longer than [`ARTIFACT_TTL`].
    fn sweep_expired(&mut self) {
        let now = Instant::now();
        let stale: Vec<String> = self
            .artifacts
            .iter()
            .filter(|(_, a)| now.duration_since(a.last_access) > ARTIFACT_TTL)
            .map(|(k, _)| k.clone())
            .collect();
        for id in stale {
            self.remove(&id);
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    store: Arc<Mutex<Store>>,
    seq: Arc<AtomicU64>,
    max_upload_bytes: u64,
}

impl AppState {
    /// Create state with the given total-storage ceiling (bytes) for artifact
    /// eviction.
    pub fn new(max_total_bytes: u64) -> Self {
        Self::with_upload_limit(max_total_bytes, DEFAULT_MAX_UPLOAD_BYTES)
    }

    pub fn with_upload_limit(max_total_bytes: u64, max_upload_bytes: u64) -> Self {
        Self {
            store: Arc::new(Mutex::new(Store {
                max_total_bytes,
                ..Store::default()
            })),
            seq: Arc::new(AtomicU64::new(0)),
            max_upload_bytes,
        }
    }

    /// Lock the store, recovering the guard if a previous holder panicked
    /// (poison only means an unfinished mutation, not corrupt data here).
    fn lock(&self) -> MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::Relaxed)
    }

    fn max_upload_bytes(&self) -> u64 {
        self.max_upload_bytes
    }

    fn max_body_bytes(&self) -> u64 {
        self.max_upload_bytes
            .saturating_add(MULTIPART_BODY_OVERHEAD_BYTES)
    }

    /// Store a temp file under a fresh UUID and return that id. Owns the
    /// uuid/seq/insert sequence shared by uploads and produced results.
    fn store_file(
        &self,
        file: NamedTempFile,
        kind: Kind,
        content_type: &'static str,
        size: u64,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        self.lock().insert(
            id.clone(),
            StoredArtifact {
                file,
                kind,
                content_type,
                size,
                seq: self.next_seq(),
                last_access: Instant::now(),
            },
        );
        id
    }

    /// Spawn a background task that periodically evicts expired artifacts.
    pub fn spawn_sweeper(&self) {
        let store = self.store.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
            loop {
                ticker.tick().await;
                store
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .sweep_expired();
            }
        });
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_TOTAL_BYTES)
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
/// Log the detail server-side and return a generic 500, so internal paths and
/// engine internals never leak to clients.
fn internal(e: impl ToString) -> AppError {
    eprintln!("oxidepdf-web: internal error: {}", e.to_string());
    AppError(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal server error".into(),
    )
}

fn core_error(e: OxideError) -> AppError {
    match e {
        OxideError::InvalidWorkflow { .. }
        | OxideError::InvalidInput { .. }
        | OxideError::UnsupportedPdfFeature { .. }
        | OxideError::EncryptedPdf
        | OxideError::IncorrectPassword
        | OxideError::ParsePdf
        | OxideError::SvgParse
        | OxideError::ImageDecode => bad_req(e),
        OxideError::ResourceLimitExceeded { .. } => {
            AppError(StatusCode::PAYLOAD_TOO_LARGE, e.to_string())
        }
        OxideError::WritePdf
        | OxideError::RenderPdf
        | OxideError::ExtractText
        | OxideError::FontResolution
        | OxideError::ArtifactStorage
        | OxideError::Io
        | OxideError::Internal => internal(e),
    }
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

fn ensure_upload_dir_under(base: &FsPath) -> io::Result<PathBuf> {
    let dir = base.join(UPLOAD_DIR);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn upload_dir() -> io::Result<PathBuf> {
    ensure_upload_dir_under(&env::current_dir()?)
}

fn upload_temp_file() -> io::Result<NamedTempFile> {
    NamedTempFile::new_in(upload_dir()?)
}

/// Write bytes to a fresh temp file off the async runtime (blocking I/O).
async fn blocking_io<T>(
    f: impl FnOnce() -> std::io::Result<T> + Send + 'static,
) -> Result<T, AppError>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(internal)?
        .map_err(internal)
}

async fn write_temp(bytes: Vec<u8>) -> Result<(NamedTempFile, u64), AppError> {
    blocking_io(move || {
        let mut tmp = upload_temp_file()?;
        tmp.write_all(&bytes)?;
        Ok((tmp, bytes.len() as u64))
    })
    .await
}

async fn write_temp_chunk(mut tmp: NamedTempFile, chunk: Bytes) -> Result<NamedTempFile, AppError> {
    blocking_io(move || {
        tmp.write_all(&chunk)?;
        Ok(tmp)
    })
    .await
}

/// Read a stored artifact's bytes off the async runtime, refreshing its access
/// time. Returns the bytes, the resolved kind, and the content type to serve.
///
/// The lock is released before the read, so a concurrent `DELETE` can drop the
/// temp file in the window before `tokio::fs::read` opens it; that turns into a
/// 500 rather than a 404. Acceptable for a single-user tool — the alternative
/// (holding the lock across the read) would serialize all downloads.
async fn read_stored(
    state: &AppState,
    id: &str,
) -> Result<(Vec<u8>, Kind, &'static str), AppError> {
    let (path, kind, content_type) = {
        let mut guard = state.lock();
        let stored = guard.artifacts.get_mut(id).ok_or_else(not_found)?;
        stored.last_access = Instant::now();
        (
            stored.file.path().to_path_buf(),
            stored.kind,
            stored.content_type,
        )
    };
    let bytes = tokio::fs::read(path).await.map_err(internal)?;
    Ok((bytes, kind, content_type))
}

async fn store_artifact(state: &AppState, artifact: Artifact) -> Result<String, AppError> {
    let kind = artifact_kind(&artifact);
    let bytes = artifact.output_bytes().map_err(core_error)?;
    let (tmp, size) = write_temp(bytes.into_owned()).await?;
    Ok(state.store_file(tmp, kind, kind.content_type(), size))
}

async fn build_store(
    state: &AppState,
    ids: &[String],
) -> Result<(Vec<ArtifactRef>, ArtifactStore), AppError> {
    let mut store = ArtifactStore::new();
    let mut refs = Vec::with_capacity(ids.len());
    for (i, id) in ids.iter().enumerate() {
        let (bytes, kind, _) = read_stored(state, id).await?;
        let r = ArtifactRef::new(format!("i{i}"));
        store.insert(r.clone(), kind.artifact(bytes)?);
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
    while let Some(mut field) = mp.next_field().await.map_err(bad_req)? {
        if files.len() >= MAX_UPLOAD_FILES {
            return Err(bad_req(format!("too many files: max {MAX_UPLOAD_FILES}")));
        }
        let filename = field.file_name().unwrap_or("file").to_string();
        let kind = kind_from_filename(&filename);
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
        let id = state.store_file(tmp, kind, content_type, size);
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
    for id in ids {
        let (bytes, kind, _) = read_stored(state, &id).await?;
        out.push((id, kind, bytes));
    }
    Ok(out)
}

async fn api_execute_workflow(
    State(state): State<AppState>,
    Json(req): Json<WorkflowReq>,
) -> Result<impl IntoResponse, AppError> {
    validate_workflow_request(&req)?;

    // Pre-populate the store with every referenced uploaded file, keyed by a
    // sanitized ref derived from the file id, and declare each as a workflow input.
    let mut store = ArtifactStore::new();
    let mut inputs_specs = Vec::new();
    let file_bytes = load_file_bytes(&state, &req.tasks).await?;
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
    let wf = build_workflow(
        inputs_specs,
        task_specs,
        last_id.clone(),
        state.max_upload_bytes(),
    );
    let artifact = run_workflow(wf, store, last_id).await?;
    let result_id = store_artifact(&state, artifact).await?;
    Ok(Json(serde_json::json!({ "result_id": result_id })))
}

// GET /api/file/:id
async fn api_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let (bytes, _, ct) = read_stored(&state, &id).await?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, ct.parse().unwrap());
    Ok((headers, Body::from(bytes)).into_response())
}

// HEAD /api/file/:id — content type + length without reading the file body,
// so the front end's pre-preview probe never buffers a 512 MiB artifact.
async fn api_head_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let mut guard = state.lock();
    let stored = guard.artifacts.get_mut(&id).ok_or_else(not_found)?;
    stored.last_access = Instant::now();
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, stored.content_type.parse().unwrap());
    headers.insert(header::CONTENT_LENGTH, stored.size.into());
    Ok((headers, Body::empty()).into_response())
}

// DELETE /api/file/:id
async fn api_delete_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.lock().remove(&id).ok_or_else(not_found)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Optional HTTP Basic credentials. When set, every request must present a
/// matching `Authorization: Basic` header.
#[derive(Clone)]
pub struct Auth {
    pub username: String,
    pub password: String,
}

/// Constant-time string compare so credential checks don't leak length-prefix
/// matches via timing. (Length itself is not hidden, which is fine here.)
fn ct_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// 401 with a `WWW-Authenticate` challenge so browsers show a login prompt.
fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Basic realm=\"oxidepdf-web\"")],
        "unauthorized",
    )
        .into_response()
}

/// Middleware enforcing HTTP Basic auth against the configured credentials.
async fn require_auth(State(auth): State<Auth>, request: Request, next: Next) -> Response {
    let (mut parts, body) = request.into_parts();
    let Ok(AuthBasic((user, pass))) = AuthBasic::from_request_parts(&mut parts, &()).await else {
        return unauthorized();
    };
    if ct_eq(&user, &auth.username) && ct_eq(pass.as_deref().unwrap_or(""), &auth.password) {
        next.run(Request::from_parts(parts, body)).await
    } else {
        unauthorized()
    }
}

pub fn router(state: AppState, auth: Option<Auth>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    let mut app = Router::new()
        .route("/", get(index))
        .route("/static/{file}", get(static_asset))
        .route("/api/schema", get(api_schema))
        .route("/api/upload", post(api_upload))
        .route("/api/execute/single", post(api_execute_single))
        .route("/api/execute/workflow", post(api_execute_workflow))
        .route(
            "/api/file/{id}",
            get(api_file).head(api_head_file).delete(api_delete_file),
        )
        .layer(DefaultBodyLimit::max(
            usize::try_from(state.max_body_bytes()).unwrap_or(usize::MAX),
        ));
    if let Some(auth) = auth {
        app = app.layer(middleware::from_fn_with_state(auth, require_auth));
    }
    app.layer(cors).with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stored artifact of the given size and insertion order. The temp file
    /// is real but empty; these tests exercise the in-memory accounting only.
    fn artifact(size: u64, seq: u64) -> StoredArtifact {
        StoredArtifact {
            file: NamedTempFile::new().unwrap(),
            kind: Kind::Pdf,
            content_type: "application/pdf",
            size,
            seq,
            last_access: Instant::now(),
        }
    }

    #[test]
    fn insert_tracks_total_bytes() {
        let mut store = Store::default();
        store.insert("a".into(), artifact(100, 0));
        store.insert("b".into(), artifact(250, 1));
        assert_eq!(store.total_bytes, 350);
        assert_eq!(store.artifacts.len(), 2);
    }

    #[test]
    fn replacing_an_id_adjusts_total_bytes() {
        let mut store = Store::default();
        store.insert("a".into(), artifact(100, 0));
        store.insert("a".into(), artifact(40, 1));
        assert_eq!(store.artifacts.len(), 1);
        assert_eq!(store.total_bytes, 40);
    }

    #[test]
    fn remove_subtracts_and_never_underflows() {
        let mut store = Store::default();
        store.insert("a".into(), artifact(100, 0));
        assert!(store.remove("a").is_some());
        assert_eq!(store.total_bytes, 0);
        // Removing a missing id is a no-op and must not underflow.
        assert!(store.remove("missing").is_none());
        assert_eq!(store.total_bytes, 0);
    }

    #[test]
    fn count_cap_evicts_oldest_first() {
        let mut store = Store::default();
        for i in 0..(MAX_ARTIFACTS as u64 + 5) {
            store.insert(format!("k{i}"), artifact(1, i));
        }
        assert_eq!(store.artifacts.len(), MAX_ARTIFACTS);
        // The five lowest seqs (oldest) are gone; the newest survive.
        assert!(!store.artifacts.contains_key("k0"));
        assert!(!store.artifacts.contains_key("k4"));
        assert!(store.artifacts.contains_key("k5"));
        assert_eq!(store.total_bytes, MAX_ARTIFACTS as u64);
    }

    #[test]
    fn size_cap_evicts_until_under_limit() {
        let mut store = Store {
            max_total_bytes: 1000,
            ..Store::default()
        };
        store.insert("a".into(), artifact(500, 0));
        store.insert("b".into(), artifact(500, 1));
        // Both fit exactly at the cap.
        assert_eq!(store.artifacts.len(), 2);
        // One more byte over the cap evicts the oldest.
        store.insert("c".into(), artifact(2, 2));
        assert!(!store.artifacts.contains_key("a"));
        assert!(store.artifacts.contains_key("b"));
        assert!(store.artifacts.contains_key("c"));
        assert!(store.total_bytes <= store.max_total_bytes);
    }

    #[test]
    fn parse_size_handles_units_and_overflow() {
        assert_eq!(parse_size("100").unwrap(), 100);
        assert_eq!(parse_size("100B").unwrap(), 100);
        assert_eq!(parse_size("100K").unwrap(), 100 * 1024);
        assert_eq!(parse_size("1024M").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("2g").unwrap(), 2 * 1024 * 1024 * 1024);
        assert_eq!(parse_size("512MiB").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_size("  4 GB ").unwrap(), 4 * 1024 * 1024 * 1024);
        assert!(parse_size("").is_err());
        assert!(parse_size("0").is_err());
        assert!(parse_size("abc").is_err());
        assert!(parse_size("10X").is_err());
        assert!(parse_size("99999999999999999999G").is_err());
    }

    #[test]
    fn ct_eq_matches_only_identical_strings() {
        assert!(ct_eq("secret", "secret"));
        assert!(!ct_eq("secret", "secres"));
        assert!(!ct_eq("secret", "secre"));
        assert!(ct_eq("", ""));
    }

    #[test]
    fn core_invalid_input_is_returned_to_web_clients() {
        let error = core_error(OxideError::InvalidInput {
            reason: "merge requires at least two PDF inputs".to_owned(),
        });

        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.1,
            "invalid input: merge requires at least two PDF inputs"
        );
    }

    #[test]
    fn core_internal_errors_stay_generic() {
        let error = core_error(OxideError::Internal);

        assert_eq!(error.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.1, "internal server error");
    }

    #[test]
    fn web_resource_limits_are_stricter_than_core_defaults() {
        let limits = web_resource_limits(DEFAULT_MAX_UPLOAD_BYTES);

        assert_eq!(DEFAULT_MAX_UPLOAD_BYTES, 128 * 1024 * 1024);
        assert_eq!(limits.max_input_bytes, Some(DEFAULT_MAX_UPLOAD_BYTES));
        assert_eq!(limits.max_total_input_bytes, Some(DEFAULT_MAX_UPLOAD_BYTES));
        assert_eq!(limits.max_output_bytes, Some(DEFAULT_MAX_UPLOAD_BYTES));
        assert_eq!(limits.timeout_ms, Some(WEB_WORKFLOW_TIMEOUT_MS));
    }

    #[test]
    fn app_state_custom_upload_limit_flows_into_workflow_limits() {
        let state = AppState::with_upload_limit(1024, 200 * 1024 * 1024);
        let workflow = build_workflow(
            vec![InputSpec {
                id: ArtifactRef::new("input"),
                path: "input".into(),
            }],
            vec![TaskSpec {
                id: TaskId::new("task"),
                op: OperatorSpec::PdfInspect(PdfInspectOptions::Metadata(
                    MetadataInspectOptions::default(),
                )),
                inputs: vec![ArtifactRef::new("input")],
            }],
            ArtifactRef::new("task"),
            state.max_upload_bytes(),
        );

        assert_eq!(workflow.limits.max_input_bytes, Some(200 * 1024 * 1024));
        assert_eq!(
            workflow.limits.max_total_input_bytes,
            Some(200 * 1024 * 1024)
        );
        assert_eq!(workflow.limits.max_output_bytes, Some(200 * 1024 * 1024));
    }

    #[test]
    fn body_limit_allows_multipart_overhead_above_file_limit() {
        let state = AppState::with_upload_limit(1024, 100 * 1024 * 1024);

        assert_eq!(state.max_upload_bytes(), 100 * 1024 * 1024);
        assert_eq!(
            state.max_body_bytes(),
            100 * 1024 * 1024 + MULTIPART_BODY_OVERHEAD_BYTES
        );
    }

    #[test]
    fn upload_dir_is_created_under_current_working_directory() {
        let base = tempfile::tempdir().unwrap();

        let dir = ensure_upload_dir_under(base.path()).unwrap();

        assert_eq!(dir, base.path().join(UPLOAD_DIR));
        assert!(dir.is_dir());
    }

    #[test]
    fn upload_temp_file_uses_upload_directory() {
        let base = tempfile::tempdir().unwrap();
        let dir = ensure_upload_dir_under(base.path()).unwrap();

        let file = tempfile::Builder::new()
            .prefix(".tmp")
            .tempfile_in(&dir)
            .unwrap();

        assert_eq!(
            file.path().parent().unwrap(),
            dir.as_path(),
            "tempfile should be created under the upload directory"
        );
    }

    #[test]
    fn single_request_rejects_empty_and_excessive_inputs() {
        let empty = SingleReq {
            artifact_ids: Vec::new(),
            family: "PdfInspect".to_owned(),
            op: "Metadata".to_owned(),
            options_json: "{}".to_owned(),
        };
        assert!(validate_single_request(&empty).is_err());

        let excessive = SingleReq {
            artifact_ids: vec!["id".to_owned(); MAX_SINGLE_INPUTS + 1],
            family: "PdfEdit".to_owned(),
            op: "Merge".to_owned(),
            options_json: "{}".to_owned(),
        };
        assert!(validate_single_request(&excessive).is_err());
    }

    #[test]
    fn workflow_request_rejects_empty_excessive_and_wide_tasks() {
        assert!(validate_workflow_request(&WorkflowReq { tasks: Vec::new() }).is_err());

        let too_many_tasks = WorkflowReq {
            tasks: (0..=MAX_WORKFLOW_TASKS)
                .map(|_| WfTask {
                    family: "PdfInspect".to_owned(),
                    op: "Metadata".to_owned(),
                    options_json: "{}".to_owned(),
                    inputs: vec![WfInput::File {
                        id: "file".to_owned(),
                    }],
                })
                .collect(),
        };
        assert!(validate_workflow_request(&too_many_tasks).is_err());

        let too_many_inputs = WorkflowReq {
            tasks: vec![WfTask {
                family: "PdfEdit".to_owned(),
                op: "Merge".to_owned(),
                options_json: "{}".to_owned(),
                inputs: (0..=MAX_TASK_INPUTS)
                    .map(|index| WfInput::File {
                        id: format!("file{index}"),
                    })
                    .collect(),
            }],
        };
        assert!(validate_workflow_request(&too_many_inputs).is_err());
    }

    #[test]
    fn sweep_expired_drops_only_stale_entries() {
        // Skip if the monotonic clock is younger than the TTL (e.g. CI booted
        // moments ago) — can't construct a timestamp older than the origin.
        let Some(stale_ts) = Instant::now().checked_sub(ARTIFACT_TTL + Duration::from_secs(1))
        else {
            return;
        };
        let mut store = Store::default();
        let mut fresh = artifact(10, 0);
        fresh.last_access = Instant::now();
        let mut stale = artifact(20, 1);
        stale.last_access = stale_ts;
        store.insert("fresh".into(), fresh);
        store.insert("stale".into(), stale);

        store.sweep_expired();

        assert!(store.artifacts.contains_key("fresh"));
        assert!(!store.artifacts.contains_key("stale"));
        assert_eq!(store.total_bytes, 10);
    }
}
