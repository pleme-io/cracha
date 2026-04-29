// HelmRelease watcher — auto-derives ServiceCatalog entries from
// observed FluxCD HelmRelease resources labeled
// `app.kubernetes.io/part-of=saguao-service`.
//
// This closes the duplicate-authoring seam where adding a service used
// to require both a lareira HelmRelease AND a separate ServiceCatalog
// CRD entry. The HelmRelease alone is now the source of truth.
//
// Contract — the lareira chart's metadata MUST stamp:
//   labels:
//     app.kubernetes.io/part-of: saguao-service
//     app.kubernetes.io/name: <slug>          # required, used as service slug
//     app.kubernetes.io/instance: <release>   # informational
//     pleme.io/cluster: <cluster>             # required, names the cluster
//   annotations:
//     saguao.pleme.io/display-name: "Vaultwarden"   # required
//     saguao.pleme.io/icon: "https://..."           # optional
//     saguao.pleme.io/description: "Password manager" # optional
//
// Annotations are preferred over values.yaml extraction because they
// survive Helm rendering + are queryable via kubectl.

use crate::index::SharedIndex;
use cracha_core::ServiceEntry;
use futures::StreamExt;
use kube::{
    api::{Api, ApiResource, DynamicObject, GroupVersionKind, ListParams},
    runtime::{controller::Action, watcher, Controller},
    Client,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const LABEL_PART_OF: &str = "app.kubernetes.io/part-of";
const LABEL_PART_OF_VALUE: &str = "saguao-service";
const LABEL_NAME: &str = "app.kubernetes.io/name";
const LABEL_CLUSTER: &str = "pleme.io/cluster";
const ANN_DISPLAY_NAME: &str = "saguao.pleme.io/display-name";
const ANN_ICON: &str = "saguao.pleme.io/icon";
const ANN_DESCRIPTION: &str = "saguao.pleme.io/description";

/// Errors raised during HelmRelease reconciliation.
#[derive(Debug, thiserror::Error)]
pub enum HelmReleaseError {
    #[error("kube client error: {0}")]
    Kube(#[from] kube::Error),
}

/// Convert one observed HelmRelease into a ServiceEntry, if it has the
/// required label + annotation set. Returns None for HelmReleases that
/// aren't saguão services (most of them).
fn to_service_entry(hr: &DynamicObject) -> Option<ServiceEntry> {
    let labels = hr.metadata.labels.as_ref()?;
    if labels.get(LABEL_PART_OF).map(String::as_str) != Some(LABEL_PART_OF_VALUE) {
        return None;
    }

    let slug = labels.get(LABEL_NAME)?.clone();
    let cluster = labels.get(LABEL_CLUSTER)?.clone();

    let annotations = hr.metadata.annotations.as_ref()?;
    let display_name = annotations.get(ANN_DISPLAY_NAME)?.clone();

    Some(ServiceEntry {
        slug,
        display_name,
        cluster,
        icon: annotations.get(ANN_ICON).cloned(),
        description: annotations.get(ANN_DESCRIPTION).cloned(),
    })
}

/// Context for the HelmRelease reconciler.
#[derive(Clone)]
pub struct HelmContext {
    pub client: Client,
    pub index: SharedIndex,
}

/// On any HelmRelease event, rebuild the auto-derived service entries
/// portion of the index. The CRD-authored service entries from
/// ServiceCatalog merge alongside (CRD wins on slug+cluster collision).
pub async fn reconcile(
    _obj: Arc<DynamicObject>,
    ctx: Arc<HelmContext>,
) -> Result<Action, HelmReleaseError> {
    rebuild_from_helmreleases(&ctx).await?;
    Ok(Action::requeue(Duration::from_secs(300)))
}

pub fn error_policy(
    _obj: Arc<DynamicObject>,
    err: &HelmReleaseError,
    _ctx: Arc<HelmContext>,
) -> Action {
    error!(error = %err, "HelmRelease reconcile error");
    Action::requeue(Duration::from_secs(30))
}

async fn rebuild_from_helmreleases(ctx: &HelmContext) -> Result<(), HelmReleaseError> {
    let gvk = GroupVersionKind::gvk("helm.toolkit.fluxcd.io", "v2", "HelmRelease");
    let api_resource = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::all_with(ctx.client.clone(), &api_resource);

    let list = api.list(&ListParams::default()).await?;
    let mut auto_entries: Vec<ServiceEntry> = Vec::new();
    for hr in list.items {
        if let Some(entry) = to_service_entry(&hr) {
            debug!(slug = %entry.slug, cluster = %entry.cluster, "auto-derived service entry");
            auto_entries.push(entry);
        }
    }

    // Merge with the explicit ServiceCatalog already in the index
    // (CRD-authored entries win on (slug, cluster) collision).
    let mut idx = ctx.index.write().await;
    let crd_entries = idx.catalog.services.clone();
    let crd_keys: std::collections::HashSet<(String, String)> = crd_entries
        .iter()
        .map(|e| (e.slug.clone(), e.cluster.clone()))
        .collect();

    let merged: Vec<ServiceEntry> = crd_entries
        .into_iter()
        .chain(
            auto_entries
                .into_iter()
                .filter(|e| !crd_keys.contains(&(e.slug.clone(), e.cluster.clone()))),
        )
        .collect();

    let merged_count = merged.len();
    idx.catalog.services = merged;
    info!(
        services = merged_count,
        "service catalog merged (CRD + auto-derived from HelmReleases)"
    );

    Ok(())
}

/// Spawn the HelmRelease controller loop. Returns when the controller
/// exits (i.e., never in normal operation).
pub async fn run(ctx: Arc<HelmContext>) {
    let gvk = GroupVersionKind::gvk("helm.toolkit.fluxcd.io", "v2", "HelmRelease");
    let api_resource = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::all_with(ctx.client.clone(), &api_resource);

    Controller::new_with(api, watcher::Config::default(), api_resource)
        .run(reconcile, error_policy, ctx)
        .for_each(|res| async move {
            if let Err(e) = res {
                error!(error = %e, "HelmRelease controller error");
            }
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use kube::api::{ApiResource, DynamicObject, ObjectMeta};
    use std::collections::BTreeMap;

    fn helm_release(
        labels: BTreeMap<String, String>,
        annotations: BTreeMap<String, String>,
    ) -> DynamicObject {
        let gvk = GroupVersionKind::gvk("helm.toolkit.fluxcd.io", "v2", "HelmRelease");
        let ar = ApiResource::from_gvk(&gvk);
        let mut obj = DynamicObject::new("test", &ar);
        obj.metadata = ObjectMeta {
            name: Some("test".into()),
            namespace: Some("default".into()),
            labels: Some(labels),
            annotations: Some(annotations),
            ..Default::default()
        };
        obj
    }

    #[test]
    fn extracts_service_entry_from_well_labeled_helmrelease() {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_PART_OF.into(), LABEL_PART_OF_VALUE.into());
        labels.insert(LABEL_NAME.into(), "vault".into());
        labels.insert(LABEL_CLUSTER.into(), "rio".into());

        let mut anns = BTreeMap::new();
        anns.insert(ANN_DISPLAY_NAME.into(), "Vaultwarden".into());
        anns.insert(ANN_ICON.into(), "https://example.com/vault.svg".into());
        anns.insert(ANN_DESCRIPTION.into(), "Password manager".into());

        let hr = helm_release(labels, anns);
        let entry = to_service_entry(&hr).unwrap();
        assert_eq!(entry.slug, "vault");
        assert_eq!(entry.display_name, "Vaultwarden");
        assert_eq!(entry.cluster, "rio");
        assert_eq!(entry.icon.as_deref(), Some("https://example.com/vault.svg"));
        assert_eq!(entry.description.as_deref(), Some("Password manager"));
    }

    #[test]
    fn skips_helmrelease_without_part_of_label() {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_NAME.into(), "vault".into());
        let anns = BTreeMap::new();
        let hr = helm_release(labels, anns);
        assert!(to_service_entry(&hr).is_none());
    }

    #[test]
    fn skips_helmrelease_with_wrong_part_of_value() {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_PART_OF.into(), "something-else".into());
        labels.insert(LABEL_NAME.into(), "vault".into());
        let anns = BTreeMap::new();
        let hr = helm_release(labels, anns);
        assert!(to_service_entry(&hr).is_none());
    }

    #[test]
    fn skips_helmrelease_missing_display_name_annotation() {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_PART_OF.into(), LABEL_PART_OF_VALUE.into());
        labels.insert(LABEL_NAME.into(), "vault".into());
        labels.insert(LABEL_CLUSTER.into(), "rio".into());
        let anns = BTreeMap::new();
        let hr = helm_release(labels, anns);
        assert!(to_service_entry(&hr).is_none());
    }

    #[test]
    fn skips_helmrelease_missing_cluster_label() {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_PART_OF.into(), LABEL_PART_OF_VALUE.into());
        labels.insert(LABEL_NAME.into(), "vault".into());
        let mut anns = BTreeMap::new();
        anns.insert(ANN_DISPLAY_NAME.into(), "Vaultwarden".into());
        let hr = helm_release(labels, anns);
        assert!(to_service_entry(&hr).is_none());
    }
}

