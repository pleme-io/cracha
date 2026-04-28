// AccessPolicy + ServiceCatalog reconciler.
//
// Watches both CRDs cluster-wide; on any change rebuilds the
// in-memory AuthzIndex. The reconcile fn is intentionally trivial
// — there is no per-resource state to update, no finalizers, no
// owner references. crachá's authority is purely the union of
// observed CRDs.

use crate::index::{replace, SharedIndex};
use cracha_core::{AccessPolicy, AccessPolicySpec, ServiceCatalog, ServiceCatalogSpec};
use futures::StreamExt;
use kube::{
    api::{Api, ListParams},
    runtime::{controller::Action, watcher, Controller},
    Client,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Context passed to the reconcile fn.
#[derive(Clone)]
pub struct Context {
    pub client: Client,
    pub index: SharedIndex,
}

/// Errors raised during reconciliation. Reconcile is idempotent
/// (full rebuild on every event) so errors here just cause a retry.
#[derive(Debug, thiserror::Error)]
pub enum ReconcileError {
    #[error("kube client error: {0}")]
    Kube(#[from] kube::Error),
}

/// Reconcile entry point. Triggered on any AccessPolicy event;
/// rebuilds the entire index from the live cluster state.
pub async fn reconcile(
    _obj: Arc<AccessPolicy>,
    ctx: Arc<Context>,
) -> Result<Action, ReconcileError> {
    rebuild_index(&ctx).await?;
    // Nothing to requeue periodically — events drive everything.
    Ok(Action::requeue(Duration::from_secs(300)))
}

/// Reconcile entry point for ServiceCatalog changes — same shape.
pub async fn reconcile_catalog(
    _obj: Arc<ServiceCatalog>,
    ctx: Arc<Context>,
) -> Result<Action, ReconcileError> {
    rebuild_index(&ctx).await?;
    Ok(Action::requeue(Duration::from_secs(300)))
}

/// Pull the current state of every AccessPolicy + the singleton
/// ServiceCatalog and rebuild the index.
async fn rebuild_index(ctx: &Context) -> Result<(), ReconcileError> {
    let policy_api: Api<AccessPolicy> = Api::all(ctx.client.clone());
    let catalog_api: Api<ServiceCatalog> = Api::all(ctx.client.clone());

    let policies_list = policy_api.list(&ListParams::default()).await?;
    let catalogs_list = catalog_api.list(&ListParams::default()).await?;

    let mut policy_map: HashMap<String, AccessPolicySpec> = HashMap::new();
    for ap in policies_list.items {
        let name = ap.spec.name.clone();
        if policy_map.contains_key(&name) {
            warn!(policy_name = %name, "duplicate AccessPolicy name observed; last write wins");
        }
        policy_map.insert(name, ap.spec);
    }

    // The catalog is conventionally a singleton — if the operator
    // declares more than one we union everything.
    let catalog: ServiceCatalogSpec = catalogs_list
        .items
        .into_iter()
        .map(|c| c.spec)
        .reduce(|mut acc, next| {
            acc.clusters.extend(next.clusters);
            acc.services.extend(next.services);
            acc
        })
        .unwrap_or_default();

    let policy_count = policy_map.len();
    let cluster_count = catalog.clusters.len();
    let service_count = catalog.services.len();

    replace(&ctx.index, policy_map, catalog).await;

    info!(
        policies = policy_count,
        clusters = cluster_count,
        services = service_count,
        "authz index rebuilt"
    );

    Ok(())
}

/// On reconcile error, requeue with backoff.
pub fn error_policy(_obj: Arc<AccessPolicy>, err: &ReconcileError, _ctx: Arc<Context>) -> Action {
    error!(error = %err, "reconcile error");
    Action::requeue(Duration::from_secs(30))
}

pub fn error_policy_catalog(
    _obj: Arc<ServiceCatalog>,
    err: &ReconcileError,
    _ctx: Arc<Context>,
) -> Action {
    error!(error = %err, "catalog reconcile error");
    Action::requeue(Duration::from_secs(30))
}

/// Spawn the controller loops for AccessPolicy + ServiceCatalog.
/// Returns when both controllers exit (i.e., never, in normal operation).
pub async fn run(ctx: Arc<Context>) {
    let policy_api: Api<AccessPolicy> = Api::all(ctx.client.clone());
    let catalog_api: Api<ServiceCatalog> = Api::all(ctx.client.clone());

    let policy_loop = Controller::new(policy_api, watcher::Config::default())
        .run(reconcile, error_policy, ctx.clone())
        .for_each(|res| async move {
            if let Err(e) = res {
                error!(error = %e, "policy controller error");
            }
        });

    let catalog_loop = Controller::new(catalog_api, watcher::Config::default())
        .run(reconcile_catalog, error_policy_catalog, ctx.clone())
        .for_each(|res| async move {
            if let Err(e) = res {
                error!(error = %e, "catalog controller error");
            }
        });

    tokio::join!(policy_loop, catalog_loop);
}
