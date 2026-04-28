// The shared in-memory authz index — wrapped in Arc<RwLock<...>>
// for cross-thread access. cracha-controller writes; cracha-api reads.

use cracha_core::{AccessPolicySpec, AuthzIndex, ServiceCatalogSpec};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// The shared handle. Cheap to clone; safe to share across tasks.
pub type SharedIndex = Arc<RwLock<AuthzIndex>>;

/// Construct an empty shared index.
#[must_use]
pub fn new_shared_index() -> SharedIndex {
    Arc::new(RwLock::new(AuthzIndex::new()))
}

/// Replace the index contents with a new policy set + catalog.
pub async fn replace(
    handle: &SharedIndex,
    policies: HashMap<String, AccessPolicySpec>,
    catalog: ServiceCatalogSpec,
) {
    let mut idx = handle.write().await;
    idx.replace(policies, catalog);
}
