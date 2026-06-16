#![forbid(unsafe_code)]

use clap::Parser;
use std::net::{IpAddr, SocketAddr};

/// Web front end for OxidePDF.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Address to bind the HTTP server to.
    #[arg(long, default_value = "0.0.0.0")]
    addr: IpAddr,
    /// Port to listen on.
    #[arg(long, short, default_value_t = 19898)]
    port: u16,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let socket = SocketAddr::new(cli.addr, cli.port);
    let state = oxidepdf_web::AppState::new();
    let app = oxidepdf_web::router(state);
    let listener = tokio::net::TcpListener::bind(socket).await.unwrap();
    println!("oxidepdf-web listening on http://{socket}");
    axum::serve(listener, app).await.unwrap();
}
