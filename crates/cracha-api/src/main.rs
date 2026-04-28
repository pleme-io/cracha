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

mod grpc;
mod rest;

use std::sync::Arc;

use clap::Parser;
use cracha_controller::{new_shared_index, reconcile, Context};
use cracha_proto::CrachaServer;
use grpc::CrachaService;
use kube::Client;
use rest::RestState;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "cracha-api")]
#[command(about = "gRPC + REST authz API for the saguão fleet", long_about = None)]
struct Args {
    #[arg(long, env = "CRACHA_API_REST_ADDR", default_value = "0.0.0.0:8080")]
    rest_addr: String,

    #[arg(long, env = "CRACHA_API_GRPC_ADDR", default_value = "0.0.0.0:50051")]
    grpc_addr: String,
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

    // REST server.
    let rest_addr: std::net::SocketAddr = args.rest_addr.parse()?;
    let rest_state = Arc::new(RestState { index });
    let rest_router = rest::router(rest_state);
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
