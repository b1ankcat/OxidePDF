#![forbid(unsafe_code)]

use clap::Parser;
use std::net::{IpAddr, SocketAddr};

/// Web front end for OxidePDF.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Address to bind the HTTP server to. Defaults to loopback; the server has
    /// no authentication, so binding to a non-loopback address exposes an
    /// unauthenticated service to the network.
    #[arg(long, default_value = "127.0.0.1")]
    addr: IpAddr,
    /// Port to listen on.
    #[arg(long, short, default_value_t = 19898)]
    port: u16,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let socket = SocketAddr::new(cli.addr, cli.port);
    if !cli.addr.is_loopback() {
        eprintln!(
            "WARNING: binding to {} exposes an UNAUTHENTICATED server to the network. \
             Anyone who can reach this address can upload, process, and download files. \
             Bind to 127.0.0.1 (the default) unless this is intentional.",
            cli.addr
        );
    }
    let state = oxidepdf_web::AppState::new();
    state.spawn_sweeper();
    let app = oxidepdf_web::router(state);
    let listener = tokio::net::TcpListener::bind(socket).await.unwrap();
    println!("oxidepdf-web listening on http://{socket}");
    axum::serve(listener, app).await.unwrap();
}
