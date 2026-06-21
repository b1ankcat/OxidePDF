
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
    let (file, size, ct) = open_stored_stream(&state, &id).await?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, ct.parse().unwrap());
    headers.insert(header::CONTENT_LENGTH, size.into());
    if ct == "application/octet-stream" {
        headers.insert(header::CONTENT_DISPOSITION, "attachment".parse().unwrap());
    }
    Ok((headers, Body::from_stream(ReaderStream::new(file))).into_response())
}

// HEAD /api/file/:id — content type + length without reading the file body,
// so the front end's pre-preview probe never buffers a 512 MiB artifact.
async fn api_head_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let mut guard = state.lock()?;
    let stored = guard.artifacts.get_mut(&id).ok_or_else(not_found)?;
    stored.last_access = Instant::now();
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, stored.content_type.parse().unwrap());
    headers.insert(header::CONTENT_LENGTH, stored.size.into());
    if stored.content_type == "application/octet-stream" {
        headers.insert(header::CONTENT_DISPOSITION, "attachment".parse().unwrap());
    }
    Ok((headers, Body::empty()).into_response())
}

// DELETE /api/file/:id
async fn api_delete_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.lock()?.remove(&id).ok_or_else(not_found)?;
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
    let Some(pass) = pass else {
        return unauthorized();
    };
    if ct_eq(&user, &auth.username) && ct_eq(&pass, &auth.password) {
        next.run(Request::from_parts(parts, body)).await
    } else {
        unauthorized()
    }
}

fn same_origin(headers: &HeaderMap) -> bool {
    let Some((host, host_port)) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| axum::http::Uri::try_from(format!("http://{value}")).ok())
        .and_then(|uri| {
            uri.authority()
                .map(|authority| (authority.host().to_owned(), authority.port_u16()))
        })
    else {
        return false;
    };
    let Some(origin) = headers
        .get(header::ORIGIN)
        .or_else(|| headers.get(header::REFERER))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| axum::http::Uri::try_from(value).ok())
    else {
        return true;
    };
    let Some(origin_scheme) = origin.scheme_str() else {
        return false;
    };
    if !matches!(origin_scheme, "http" | "https") {
        return false;
    }
    let Some(origin_authority) = origin.authority() else {
        return false;
    };
    let default_port = if origin_scheme.eq_ignore_ascii_case("https") {
        443
    } else {
        80
    };
    let origin_port = origin_authority.port_u16().or(Some(default_port));
    let host_port = host_port.or(Some(default_port));
    origin_authority.host().eq_ignore_ascii_case(&host) && origin_port == host_port
}

async fn require_same_origin(request: Request, next: Next) -> Response {
    if matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) && !same_origin(request.headers())
    {
        return (
            StatusCode::FORBIDDEN,
            "cross-origin state-changing request rejected",
        )
            .into_response();
    }
    next.run(request).await
}

pub fn router(state: AppState, auth: Option<Auth>) -> Result<Router, String> {
    let body_limit = match state.max_body_bytes() {
        Ok(limit) => match usize::try_from(limit) {
            Ok(limit) => limit,
            Err(_) => return Err("upload body limit does not fit in usize".to_owned()),
        },
        Err(error) => return Err(format!("invalid upload body limit: {error:?}")),
    };
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
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(middleware::from_fn(require_same_origin));
    if let Some(auth) = auth {
        app = app.layer(middleware::from_fn_with_state(auth, require_auth));
    }
    Ok(app.with_state(state))
}
