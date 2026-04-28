// axum REST surface — consumed by varanda for the portal manifest.
//
// Routes:
//   GET  /accessible-services?user=<sub>     → JSON list of services
//   GET  /healthz                            → 200 OK
//   GET  /metrics                            → Prometheus text format

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use cracha_controller::SharedIndex;
use cracha_core::AccessibleService;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct RestState {
    pub index: SharedIndex,
}

#[derive(Debug, Deserialize)]
pub struct AccessibleQuery {
    pub user: String,
}

pub fn router(state: Arc<RestState>) -> Router {
    Router::new()
        .route("/accessible-services", get(get_accessible_services))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .with_state(state)
}

async fn get_accessible_services(
    State(state): State<Arc<RestState>>,
    Query(q): Query<AccessibleQuery>,
) -> Json<Vec<AccessibleService>> {
    let idx = state.index.read().await;
    Json(idx.accessible_services(&q.user))
}

async fn healthz() -> &'static str {
    "ok"
}

async fn readyz(State(state): State<Arc<RestState>>) -> Response {
    let idx = state.index.read().await;
    if idx.policy_count() == 0 {
        // No policies indexed yet — controller hasn't completed its
        // first reconcile. Return 503 so K8s waits before sending traffic.
        (StatusCode::SERVICE_UNAVAILABLE, "no policies indexed").into_response()
    } else {
        (StatusCode::OK, "ready").into_response()
    }
}

async fn metrics() -> Response {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    match encoder.encode_to_string(&metric_families) {
        Ok(body) => (StatusCode::OK, body).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
