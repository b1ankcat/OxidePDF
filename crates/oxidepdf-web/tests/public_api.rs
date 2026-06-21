use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[test]
fn parse_size_accepts_binary_units_and_rejects_invalid_input() {
    assert_eq!(oxidepdf_web::parse_size("100").unwrap(), 100);
    assert_eq!(oxidepdf_web::parse_size("100K").unwrap(), 100 * 1024);
    assert_eq!(
        oxidepdf_web::parse_size("512MiB").unwrap(),
        512 * 1024 * 1024
    );
    assert!(oxidepdf_web::parse_size("").is_err());
    assert!(oxidepdf_web::parse_size("0").is_err());
    assert!(oxidepdf_web::parse_size("99999999999999999999G").is_err());
}

#[tokio::test]
async fn router_serves_public_schema() {
    let app = oxidepdf_web::router(oxidepdf_web::AppState::new(1024 * 1024), None);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/schema")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let schema: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(schema.as_array().unwrap().iter().any(|family| {
        family["name"] == "PdfEdit"
            && family["ops"]
                .as_array()
                .unwrap()
                .iter()
                .any(|op| op["name"] == "Merge")
    }));
}

#[tokio::test]
async fn router_requires_matching_basic_auth_when_configured() {
    let auth = oxidepdf_web::Auth {
        username: "user".to_owned(),
        password: "pass".to_owned(),
    };
    let app = oxidepdf_web::router(oxidepdf_web::AppState::new(1024 * 1024), Some(auth));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/schema")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/schema")
                .header(header::AUTHORIZATION, "Basic dXNlcjpwYXNz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn router_rejects_basic_auth_without_password() {
    let auth = oxidepdf_web::Auth {
        username: "user".to_owned(),
        password: String::new(),
    };
    let app = oxidepdf_web::router(oxidepdf_web::AppState::new(1024 * 1024), Some(auth));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/schema")
                .header(header::AUTHORIZATION, "Basic dXNlcg==")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn router_rejects_uploads_without_supported_extension() {
    let app = oxidepdf_web::router(oxidepdf_web::AppState::new(1024 * 1024), None);
    let boundary = "oxidepdf-boundary";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"payload.bin\"\r\n\
         Content-Type: application/octet-stream\r\n\
         \r\n\
         bytes\r\n\
         --{boundary}--\r\n"
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/upload")
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .header(header::HOST, "127.0.0.1:19898")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(
        std::str::from_utf8(&body)
            .unwrap()
            .contains("unsupported file extension")
    );
}

#[tokio::test]
async fn router_serves_uploaded_svg_as_attachment_bytes() {
    let app = oxidepdf_web::router(oxidepdf_web::AppState::new(1024 * 1024), None);
    let boundary = "oxidepdf-boundary";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"image.svg\"\r\n\
         Content-Type: image/svg+xml\r\n\
         \r\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\"></svg>\r\n\
         --{boundary}--\r\n"
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/upload")
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .header(header::HOST, "127.0.0.1:19898")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = json["files"][0]["id"].as_str().unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri(format!("/api/file/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/octet-stream"
    );
    assert_eq!(
        response.headers().get(header::CONTENT_DISPOSITION).unwrap(),
        "attachment"
    );
}
