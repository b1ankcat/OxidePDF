#![forbid(unsafe_code)]

use clap::Parser;
use oxidepdf_web::{Auth, DEFAULT_MAX_UPLOAD_BYTES, parse_size};
use std::net::{IpAddr, SocketAddr};

/// Web front end for OxidePDF.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Address to bind the HTTP server to. Defaults to loopback; without auth,
    /// binding to a non-loopback address exposes an unauthenticated service.
    #[arg(long, env = "OXIDEPDF_ADDR", default_value = "127.0.0.1")]
    addr: IpAddr,
    /// Port to listen on.
    #[arg(long, short, env = "OXIDEPDF_PORT", default_value_t = 19898)]
    port: u16,
    /// Max total size of retained artifacts before oldest-first eviction.
    /// Accepts human sizes like `2G`, `1024M`, `100K` (binary units).
    #[arg(
        long,
        env = "OXIDEPDF_MAX_STORAGE",
        default_value = "2G",
        value_parser = parse_size
    )]
    max_storage: u64,
    /// Max size of one uploaded file and the matching workflow input/output
    /// limit. Defaults to 128 MiB; the HTTP request body allows a small
    /// multipart overhead above this.
    #[arg(
        long,
        env = "OXIDEPDF_MAX_UPLOAD",
        default_value_t = DEFAULT_MAX_UPLOAD_BYTES,
        value_parser = parse_size
    )]
    max_upload: u64,
    /// Username for HTTP Basic auth. Enables auth only when paired with
    /// --auth-pass; otherwise the server runs unauthenticated.
    #[arg(long, env = "OXIDEPDF_AUTH_USER")]
    auth_user: Option<String>,
    /// Password for HTTP Basic auth. See --auth-user.
    #[arg(long, env = "OXIDEPDF_AUTH_PASS")]
    auth_pass: Option<String>,
    /// Explicitly allow binding an unauthenticated server to a non-loopback
    /// address. Use only behind a trusted network boundary.
    #[arg(long, env = "OXIDEPDF_ALLOW_UNAUTH_NETWORK", default_value_t = false)]
    allow_unauth_network: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let socket = SocketAddr::new(cli.addr, cli.port);

    let auth = match (cli.auth_user, cli.auth_pass) {
        (Some(username), Some(password)) => Some(Auth { username, password }),
        (None, None) => None,
        _ => {
            eprintln!("error: --auth-user and --auth-pass must be set together");
            std::process::exit(2);
        }
    };

    if auth.is_none() && !cli.addr.is_loopback() && !cli.allow_unauth_network {
        eprintln!(
            "error: refusing to bind unauthenticated server to {}. \
             Set --auth-user/--auth-pass, bind to 127.0.0.1, or pass \
             --allow-unauth-network if this is intentionally protected elsewhere.",
            cli.addr
        );
        std::process::exit(2);
    }

    let state = oxidepdf_web::AppState::with_upload_limit(cli.max_storage, cli.max_upload);
    state.spawn_sweeper();
    let app = match oxidepdf_web::router(state, auth.clone()) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    };
    let listener = match tokio::net::TcpListener::bind(socket).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("error: failed to bind {socket}: {error}");
            std::process::exit(2);
        }
    };
    println!(
        "oxidepdf-web listening on http://{socket} (auth: {}, max storage: {} bytes, max upload: {} bytes)",
        if auth.is_some() { "on" } else { "off" },
        cli.max_storage,
        cli.max_upload
    );
    if let Err(error) = axum::serve(listener, app).await {
        eprintln!("error: server failed: {error}");
        std::process::exit(1);
    }
}
