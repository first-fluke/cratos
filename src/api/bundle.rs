use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{Html, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use utoipa::ToSchema;
use tracing::error;

#[derive(Serialize, ToSchema)]
pub struct BundleMeta {
    /// Semantic version of the bundle
    pub version: String,
    /// SHA256 hash of the bundle file
    pub hash: String,
    /// Size in bytes
    pub size: u64,
}

/// Router for bundle distribution
pub fn bundle_routes() -> Router {
    Router::new()
        .route("/bundle/latest", get(download_bundle))
        .route("/bundle/latest/raw", get(download_raw_bundle))
        .route("/bundle/meta", get(get_bundle_meta))
}

/// Get metadata for the latest A2UI bundle
#[utoipa::path(
    get,
    path = "/bundle/meta",
    tag = "bundle",
    responses(
        (status = 200, description = "Bundle metadata", body = BundleMeta),
        (status = 500, description = "Internal server error")
    )
)]
async fn get_bundle_meta() -> Result<Json<BundleMeta>, StatusCode> {
    // TODO: Make this path configurable via config
    let path = PathBuf::from("assets/a2ui/bundle.zip");
    
    if !path.exists() {
        error!("Bundle file not found at {:?}", path);
        return Err(StatusCode::NOT_FOUND);
    }

    let metadata = tokio::fs::metadata(&path).await.map_err(|e| {
        error!("Failed to read metadata: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut file = File::open(&path).await.map_err(|e| {
        error!("Failed to open file: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut hasher = Sha256::new();
    let mut buffer = [0; 1024];

    loop {
        let n = file.read(&mut buffer).await.map_err(|e| {
            error!("Failed to read file: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    let hash = format!("{:x}", hasher.finalize());

    Ok(Json(BundleMeta {
        version: "1.0.0".to_string(), // TODO: Read from manifest
        hash,
        size: metadata.len(),
    }))
}

/// Download the latest A2UI bundle zip
#[utoipa::path(
    get,
    path = "/bundle/latest",
    tag = "bundle",
    responses(
        (status = 200, description = "Bundle zip file", content_type = "application/zip"),
        (status = 404, description = "Bundle not found")
    )
)]
async fn download_bundle() -> Result<Response, StatusCode> {
    // TODO: Make this path configurable via config
    let path = PathBuf::from("assets/a2ui/bundle.zip");

    if !path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let file = File::open(&path).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let stream = tokio_util::io::ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"bundle.zip\"")
        .body(body)
        .unwrap())
}

/// Download the raw index.html (MVP helper)
#[utoipa::path(
    get,
    path = "/bundle/latest/raw",
    tag = "bundle",
    responses(
        (status = 200, description = "Raw HTML file", content_type = "text/html"),
        (status = 404, description = "File not found")
    )
)]
async fn download_raw_bundle() -> Result<Html<String>, StatusCode> {
    let path = PathBuf::from("assets/a2ui/index.html"); // Raw entry
    if !path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    
    let content = tokio::fs::read_to_string(&path).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(content))
}


