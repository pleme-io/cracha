// crachá-api — gRPC + REST authz API for the saguão fleet.
//
// Co-deployed with cracha-controller in a single Pod; shares the
// SharedIndex via Arc. Three surfaces, one shared index:
//   - gRPC Authorize / Health     (consumed by vigia)
//   - REST /accessible-services   (consumed by varanda)
//   - REST /healthz, /readyz, /metrics
//
// In Phase 1 (this scaffold), the controller and api binaries run
// in separate processes within the same Pod — they communicate via
// the shared SharedIndex by both calling cracha_controller::reconcile::run
// inline. A subsequent revision can split them with a small
// internal HTTP shim.

mod auth;
mod error;
mod grpc;
mod rest;
mod role;
mod routes;
mod state;

use std::collections::HashSet;
use std::sync::Arc;

use clap::Parser;
use cracha_controller::{new_shared_index, reconcile, Context};
use cracha_proto::CrachaServer;
use cracha_storage::Repo;
use grpc::CrachaService;
use kube::Client;
use rest::RestState;
use state::ApiState;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "cracha-api")]
#[command(about = "gRPC + REST authz API for the saguão fleet", long_about = None)]
struct Args {
    #[arg(long, env = "CRACHA_API_REST_ADDR", default_value = "0.0.0.0:8080")]
    rest_addr: String,

    #[arg(long, env = "CRACHA_API_GRPC_ADDR", default_value = "0.0.0.0:50051")]
    grpc_addr: String,

    /// Postgres connection URL for the identity registry. shikumi
    /// resolves the secret in production; passing here for dev.
    #[arg(
        long,
        env = "CRACHA_DATABASE_URL",
        default_value = "postgres://cracha:cracha@localhost:5432/cracha"
    )]
    database_url: String,

    /// Comma-separated admin emails. Lowercase comparison. Wins admin
    /// role on first login + every /me lookup. Source of truth is
    /// shikumi config; CLI flag exists for dev/integration tests.
    #[arg(long, env = "CRACHA_ADMIN_EMAILS", default_value = "")]
    admin_emails: String,

    /// passaporte JWKS URL — fetched once at startup, refreshed
    /// on cache miss. Empty = disable auth middleware (dev/test
    /// path; every authenticated request returns 401).
    #[arg(long, env = "CRACHA_JWKS_URL", default_value = "")]
    jwks_url: String,

    /// Expected `aud` claim value. Empty = audience check disabled
    /// (dev/test). Production: set to cracha's OIDC client_id from
    /// the Authentik blueprint.
    #[arg(long, env = "CRACHA_AUDIENCE", default_value = "")]
    audience: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    info!(?args, "cracha-api starting");

    // Build the shared index + spawn the reconciler in the same process.
    let client = Client::try_default().await?;
    let index = new_shared_index();
    let ctx = Arc::new(Context {
        client,
        index: index.clone(),
    });

    // Reconciler loop in background.
    let ctx_for_reconciler = ctx.clone();
    tokio::spawn(async move {
        reconcile::run(ctx_for_reconciler).await;
    });

    // gRPC server.
    let grpc_addr = args.grpc_addr.parse()?;
    let grpc_service = CrachaService { index: index.clone() };
    let grpc_handle = tokio::spawn(async move {
        info!(addr = %grpc_addr, "gRPC server listening");
        if let Err(e) = tonic::transport::Server::builder()
            .add_service(CrachaServer::new(grpc_service))
            .serve(grpc_addr)
            .await
        {
            tracing::error!(error = %e, "gRPC server exited");
        }
    });

    // DB connect + run pending migrations.
    let db = cracha_storage::connect(&args.database_url).await?;
    cracha_storage::migrate(&db).await?;
    let repo = Repo::new(db);

    // Admin allowlist. shikumi-loaded in prod via env; CLI fallback
    // covers tests + dev. Lowercase normalisation matches role::compute_role.
    let admin_emails: HashSet<String> = args
        .admin_emails
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if admin_emails.is_empty() {
        tracing::warn!("no admin emails configured — no user will get admin role");
    } else {
        info!(count = admin_emails.len(), "admin allowlist loaded");
    }

    // JWKS cache + audience for the auth middleware. Empty URL
    // disables auth (dev/test); auth_middleware short-circuits to
    // 401 in that case.
    let jwks = if args.jwks_url.is_empty() {
        tracing::warn!("no JWKS URL configured — authenticated routes will reject all requests");
        None
    } else {
        info!(url = %args.jwks_url, "JWKS cache configured");
        Some(auth::jwks::JwksCache::new(args.jwks_url.clone()))
    };
    let audience = if args.audience.is_empty() {
        None
    } else {
        Some(args.audience.clone())
    };

    // REST server hosts both the legacy /accessible-services route
    // (consumed by vigia today) and the new auth-flow surfaces
    // (/me, /admin/*) — the latter run through the JWT middleware.
    let rest_addr: std::net::SocketAddr = args.rest_addr.parse()?;
    let api_state = Arc::new(ApiState {
        index: index.clone(),
        repo,
        admin_emails,
        jwks,
        audience,
    });
    let rest_router = rest::router(Arc::new(RestState { index: index.clone() }))
        .merge(routes::wire(api_state));
    let rest_handle = tokio::spawn(async move {
        info!(addr = %rest_addr, "REST server listening");
        let listener = tokio::net::TcpListener::bind(rest_addr).await.unwrap();
        if let Err(e) = axum::serve(listener, rest_router).await {
            tracing::error!(error = %e, "REST server exited");
        }
    });

    let _ = tokio::join!(grpc_handle, rest_handle);
    Ok(())
}
