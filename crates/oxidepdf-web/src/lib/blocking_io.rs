
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

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Store>, AppError> {
        self.store
            .lock()
            .map_err(|_| internal("artifact store lock poisoned"))
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::Relaxed)
    }

    fn max_upload_bytes(&self) -> u64 {
        self.max_upload_bytes
    }

    fn max_body_bytes(&self) -> Result<u64, AppError> {
        self.max_upload_bytes
            .checked_add(MULTIPART_BODY_OVERHEAD_BYTES)
            .ok_or_else(|| internal("upload body limit overflow"))
    }

    /// Store a temp file under a fresh UUID and return that id. Owns the
    /// uuid/seq/insert sequence shared by uploads and produced results.
    fn store_file(
        &self,
        file: NamedTempFile,
        kind: Kind,
        content_type: &'static str,
        size: u64,
    ) -> Result<String, AppError> {
        let id = Uuid::new_v4().to_string();
        self.lock()?.insert(
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
        Ok(id)
    }

    /// Spawn a background task that periodically evicts expired artifacts.
    pub fn spawn_sweeper(&self) {
        let store = self.store.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
            loop {
                ticker.tick().await;
                match store.lock() {
                    Ok(mut store) => store.sweep_expired(),
                    Err(_) => {
                        eprintln!("oxidepdf-web: fatal error: artifact store lock poisoned");
                        std::process::abort();
                    }
                }
            }
        });
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_TOTAL_BYTES)
    }
}

#[derive(Debug)]
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
        let mut guard = state.lock()?;
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

async fn open_stored_stream(
    state: &AppState,
    id: &str,
) -> Result<(tokio::fs::File, u64, &'static str), AppError> {
    let (path, size, content_type) = {
        let mut guard = state.lock()?;
        let stored = guard.artifacts.get_mut(id).ok_or_else(not_found)?;
        stored.last_access = Instant::now();
        (
            stored.file.path().to_path_buf(),
            stored.size,
            stored.content_type,
        )
    };
    let file = tokio::fs::File::open(path).await.map_err(internal)?;
    Ok((file, size, content_type))
}

async fn store_artifact(state: &AppState, artifact: Artifact) -> Result<String, AppError> {
    let kind = artifact_kind(&artifact);
    let limits = web_resource_limits(state.max_upload_bytes());
    let (tmp, size) = blocking_io(move || {
        let mut tmp = upload_temp_file()?;
        let size = artifact
            .write_output_to(&mut tmp, &limits)
            .map_err(io::Error::other)?;
        Ok((tmp, size))
    })
    .await?;
    state.store_file(tmp, kind, kind.content_type(), size)
}

async fn build_store(
    state: &AppState,
    ids: &[String],
) -> Result<(Vec<ArtifactRef>, ArtifactStore), AppError> {
    enforce_total_stored_size(state, ids.iter().map(String::as_str))?;
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

fn enforce_total_stored_size<'a>(
    state: &AppState,
    ids: impl IntoIterator<Item = &'a str>,
) -> Result<(), AppError> {
    let guard = state.lock()?;
    let mut total = 0u64;
    for id in ids {
        let stored = guard.artifacts.get(id).ok_or_else(not_found)?;
        total = total
            .checked_add(stored.size)
            .ok_or_else(max_total_input_bytes_error)?;
        if total > state.max_upload_bytes() {
            return Err(max_total_input_bytes_error());
        }
    }
    Ok(())
}

fn max_total_input_bytes_error() -> AppError {
    core_error(OxideError::ResourceLimitExceeded {
        limit: "max_total_input_bytes".to_owned(),
    })
}
