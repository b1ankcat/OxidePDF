use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, FromRequestParts, Multipart, Path, Request, State},
    http::{HeaderMap, Method, StatusCode, header},
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
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tempfile::NamedTempFile;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

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
/// Maximum number of requests processed concurrently across the whole server.
/// Each workflow request can pin blocking threads for the workflow timeout, so
/// this bounds thread-pool and CPU pressure; excess requests are load-shed with
/// 503 rather than queueing unboundedly.
const MAX_CONCURRENT_REQUESTS: usize = 64;
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
    /// Content type for downloaded output. Image uploads carry their own precise
    /// type (see [`StoredArtifact::content_type`]); this is the default used
    /// for engine-produced artifacts where only the kind is known.
    fn content_type(self) -> &'static str {
        match self {
            Kind::Pdf => "application/pdf",
            Kind::Image => "image/png",
            Kind::Svg => "application/octet-stream",
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

fn kind_from_filename(name: &str) -> Result<Kind, AppError> {
    match extension(name).as_str() {
        "pdf" => Ok(Kind::Pdf),
        "png" | "jpg" | "jpeg" | "webp" => Ok(Kind::Image),
        "svg" => Ok(Kind::Svg),
        _ => Err(bad_req("unsupported file extension")),
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
            "png" => "image/png",
            _ => "application/octet-stream",
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
        self.total_bytes = self.total_bytes.saturating_add(artifact.size);
        if let Some(old) = self.artifacts.insert(id, artifact) {
            self.total_bytes = self.total_bytes.saturating_sub(old.size);
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
            self.total_bytes = self.total_bytes.saturating_sub(a.size);
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
