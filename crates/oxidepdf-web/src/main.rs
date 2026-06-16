#![forbid(unsafe_code)]

use clap::Parser;
use oxidepdf_web::{Auth, parse_size};
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
    /// Username for HTTP Basic auth. Enables auth only when paired with
    /// --auth-pass; otherwise the server runs unauthenticated.
    #[arg(long, env = "OXIDEPDF_AUTH_USER")]
    auth_user: Option<String>,
    /// Password for HTTP Basic auth. See --auth-user.
    #[arg(long, env = "OXIDEPDF_AUTH_PASS")]
    auth_pass: Option<String>,
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

    if auth.is_none() && !cli.addr.is_loopback() {
        eprintln!(
            "WARNING: binding to {} exposes an UNAUTHENTICATED server to the network. \
             Anyone who can reach this address can upload, process, and download files. \
             Set --auth-user/--auth-pass, or bind to 127.0.0.1, unless this is intentional.",
            cli.addr
        );
    }

    let state = oxidepdf_web::AppState::new(cli.max_storage);
    state.spawn_sweeper();
    let app = oxidepdf_web::router(state, auth.clone());
    let listener = tokio::net::TcpListener::bind(socket).await.unwrap();
    println!(
        "oxidepdf-web listening on http://{socket} (auth: {}, max storage: {} bytes)",
        if auth.is_some() { "on" } else { "off" },
        cli.max_storage
    );
    axum::serve(listener, app).await.unwrap();
}
