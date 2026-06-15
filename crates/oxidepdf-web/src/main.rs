#![forbid(unsafe_code)]

use axum::{
    Json, Router,
    extract::Multipart,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use clap::Parser;
use oxidepdf_core::{Artifact, ArtifactStore, PdfOperatorRunner, Workflow, execute_workflow};
use rust_embed::Embed;
use serde_json::{Value, json};
use std::collections::HashMap;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

#[derive(Embed)]
#[folder = "static/"]
struct Assets;

#[derive(Parser)]
#[command(version, about = "OxidePDF web server")]
struct Args {
    /// Address to bind.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    /// Port to listen on.
    #[arg(long, default_value_t = 8080)]
    port: u16,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let addr = format!("{}:{}", args.host, args.port);
    let app = Router::new()
        .route("/", get(index))
        .route("/api/run", post(run))
        .layer(CorsLayer::permissive());
    let listener = TcpListener::bind(&addr).await.expect("bind failed");
    eprintln!("oxidepdf-web listening on http://{addr}");
    axum::serve(listener, app).await.expect("serve failed");
}

async fn index() -> impl IntoResponse {
    match Assets::get("index.html") {
        Some(f) => Html(String::from_utf8_lossy(&f.data).into_owned()).into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

async fn run(mut mp: Multipart) -> Response {
    let mut workflow_src: Option<String> = None;
    let mut uploads: HashMap<String, Vec<u8>> = HashMap::new();

    while let Ok(Some(field)) = mp.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        let bytes = match field.bytes().await {
            Ok(b) => b.to_vec(),
            Err(e) => return err(StatusCode::BAD_REQUEST, &e.to_string()),
        };
        if name == "workflow" {
            workflow_src = Some(String::from_utf8_lossy(&bytes).into_owned());
        } else {
            uploads.insert(name, bytes);
        }
    }

    let src = match workflow_src {
        Some(s) => s,
        None => return err(StatusCode::BAD_REQUEST, "missing 'workflow' field"),
    };

    let workflow: Workflow = match serde_saphyr::from_str(&src) {
        Ok(w) => w,
        Err(e) => return err(StatusCode::BAD_REQUEST, &format!("invalid workflow: {e}")),
    };

    let mut store = ArtifactStore::new();
    for input in &workflow.inputs {
        let id = input.id.as_str().to_string();
        match uploads.remove(&id) {
            Some(b) => {
                let artifact = match Artifact::bytes(b) {
                    Ok(artifact) => artifact,
                    Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
                };
                store.insert(input.id.clone(), artifact);
            }
            None => {
                return err(
                    StatusCode::BAD_REQUEST,
                    &format!("missing upload for input '{id}'"),
                );
            }
        }
    }

    let runner = PdfOperatorRunner::with_limits(workflow.limits.clone());
    let result =
        match tokio::task::spawn_blocking(move || execute_workflow(&workflow, store, &runner))
            .await
            .expect("task panicked")
        {
            Ok(r) => r,
            Err(e) => return err(StatusCode::UNPROCESSABLE_ENTITY, &e.to_string()),
        };

    let mut outputs: HashMap<String, Value> = HashMap::new();
    for output in &result.plan.output_refs {
        if let Some(artifact) = result.store.get(output) {
            match artifact.output_bytes() {
                Ok(bytes) => {
                    outputs.insert(output.as_str().to_string(), json!(B64.encode(&*bytes)));
                }
                Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            }
        }
    }

    Json(json!({ "outputs": outputs })).into_response()
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}
