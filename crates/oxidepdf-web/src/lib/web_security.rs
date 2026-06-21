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
    if ct_eq(&user, &auth.username) & ct_eq(&pass, &auth.password) {
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
    let origin_header = headers
        .get(header::ORIGIN)
        .or_else(|| headers.get(header::REFERER));
    let Some(origin_header) = origin_header else {
        // No Origin/Referer at all: a non-browser client (curl, native app).
        // Browsers always send one on cross-origin state-changing requests, so
        // its absence is allowed; its presence must validate.
        return true;
    };
    let Some(origin) = origin_header
        .to_str()
        .ok()
        .and_then(|value| axum::http::Uri::try_from(value).ok())
    else {
        // Present but unparseable: fail closed rather than allowing it through.
        return false;
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

/// Rejects requests whose `Host` header is not in the configured allowlist.
/// `same_origin` trusts the `Host` header as ground truth, so without this a DNS
/// rebinding attack (attacker domain resolving to the bound address) sends a
/// matching `Host` and `Origin` and slips past the same-origin check. The
/// allowlist pins the acceptable host names to those the server was bound for.
async fn require_allowed_host(
    State(allowed): State<AllowedHosts>,
    request: Request,
    next: Next,
) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .map(|value| value.to_str().map(host_without_port));
    let permitted = match host {
        // No Host header (HTTP/1.0 or a non-browser client): a browser always
        // sends one, so its absence cannot be a rebinding attack — allow it.
        None => true,
        // Present but not valid UTF-8: fail closed.
        Some(Err(_)) => false,
        Some(Ok(host)) => allowed
            .hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host)),
    };
    if permitted {
        next.run(request).await
    } else {
        (StatusCode::FORBIDDEN, "host not allowed").into_response()
    }
}

fn host_without_port(value: &str) -> &str {
    // Strip the optional :port. Bracketed IPv6 literals ([::1]:8080) keep their
    // brackets ("[::1]"), matching how the allowlist stores them.
    if value.starts_with('[') {
        return match value.find(']') {
            Some(close) => &value[..=close],
            None => value,
        };
    }
    value.split(':').next().unwrap_or(value)
}

/// Host names accepted in the `Host` header. Defaults to loopback names.
#[derive(Clone)]
pub struct AllowedHosts {
    hosts: Vec<String>,
}

impl AllowedHosts {
    /// Builds an allowlist from explicit host names, falling back to the
    /// loopback set when empty so the default loopback deployment works.
    pub fn new(hosts: Vec<String>) -> Self {
        let hosts = if hosts.is_empty() {
            vec![
                "localhost".to_owned(),
                "127.0.0.1".to_owned(),
                "[::1]".to_owned(),
            ]
        } else {
            hosts
        };
        Self { hosts }
    }
}
