// Cluster registry + service catalog — the "what exists" side of
// the authz model. AccessPolicy decides "who can see what"; the
// catalog tells crachá / vigia / varanda what *what* means.

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One known cluster + its location. Populated by the cluster
/// onboarding flow (one entry per `(defcluster …)` in the fleet).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ClusterRegistration {
    /// Cluster name (e.g., "rio", "mar").
    pub name: String,
    /// Location (e.g., "bristol", "parnamirim").
    pub location: String,
    /// Optional human-readable label.
    #[serde(default)]
    pub label: Option<String>,
}

/// One known service deployed to one known cluster. The portal
/// (varanda) reads this to render tiles; vigia uses it to validate
/// that the requested service is actually deployed before consulting
/// the policy.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ServiceEntry {
    /// Service slug (e.g., "vault", "photos"). Must match the
    /// `<app>` part of `<app>.<cluster>.<location>.quero.cloud`.
    pub slug: String,
    /// Display name (e.g., "Vaultwarden", "Immich").
    pub display_name: String,
    /// Cluster the service is deployed to.
    pub cluster: String,
    /// Optional icon (URL or `data:` URI).
    #[serde(default)]
    pub icon: Option<String>,
    /// Optional one-line description.
    #[serde(default)]
    pub description: Option<String>,
}

/// Spec field of the ServiceCatalog CRD — the typed inventory of
/// services available across the fleet. Singleton named `default`
/// in the saguão-control-plane namespace.
#[derive(
    Debug, Clone, Default, Serialize, Deserialize, JsonSchema, CustomResource, PartialEq, Eq,
)]
#[kube(
    group = "saguao.pleme.io",
    version = "v1alpha1",
    kind = "ServiceCatalog",
    plural = "servicecatalogs",
    shortname = "scat",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCatalogSpec {
    /// Known clusters in the fleet.
    #[serde(default)]
    pub clusters: Vec<ClusterRegistration>,
    /// Known services across the fleet.
    #[serde(default)]
    pub services: Vec<ServiceEntry>,
}

impl ServiceCatalogSpec {
    /// Look up the location for a cluster name.
    #[must_use]
    pub fn location_for_cluster(&self, cluster: &str) -> Option<&str> {
        self.clusters
            .iter()
            .find(|c| c.name == cluster)
            .map(|c| c.location.as_str())
    }

    /// All services on a given cluster.
    #[must_use]
    pub fn services_on(&self, cluster: &str) -> Vec<&ServiceEntry> {
        self.services
            .iter()
            .filter(|s| s.cluster == cluster)
            .collect()
    }

    /// All clusters at a given location.
    #[must_use]
    pub fn clusters_at(&self, location: &str) -> Vec<&ClusterRegistration> {
        self.clusters
            .iter()
            .filter(|c| c.location == location)
            .collect()
    }

    /// All known location names (deduplicated).
    #[must_use]
    pub fn locations(&self) -> Vec<String> {
        let mut locs: Vec<String> =
            self.clusters.iter().map(|c| c.location.clone()).collect();
        locs.sort();
        locs.dedup();
        locs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> ServiceCatalogSpec {
        ServiceCatalogSpec {
            clusters: vec![
                ClusterRegistration {
                    name: "rio".into(),
                    location: "bristol".into(),
                    label: Some("Bristol home edge".into()),
                },
                ClusterRegistration {
                    name: "mar".into(),
                    location: "parnamirim".into(),
                    label: None,
                },
            ],
            services: vec![
                ServiceEntry {
                    slug: "vault".into(),
                    display_name: "Vaultwarden".into(),
                    cluster: "rio".into(),
                    icon: None,
                    description: Some("Password manager".into()),
                },
                ServiceEntry {
                    slug: "photos".into(),
                    display_name: "Immich".into(),
                    cluster: "rio".into(),
                    icon: None,
                    description: None,
                },
                ServiceEntry {
                    slug: "photos".into(),
                    display_name: "Immich (Parnamirim)".into(),
                    cluster: "mar".into(),
                    icon: None,
                    description: None,
                },
            ],
        }
    }

    #[test]
    fn location_for_cluster() {
        let c = fixture();
        assert_eq!(c.location_for_cluster("rio"), Some("bristol"));
        assert_eq!(c.location_for_cluster("mar"), Some("parnamirim"));
        assert_eq!(c.location_for_cluster("unknown"), None);
    }

    #[test]
    fn services_on_cluster() {
        let c = fixture();
        assert_eq!(c.services_on("rio").len(), 2);
        assert_eq!(c.services_on("mar").len(), 1);
        assert_eq!(c.services_on("unknown").len(), 0);
    }

    #[test]
    fn clusters_at_location() {
        let c = fixture();
        assert_eq!(c.clusters_at("bristol").len(), 1);
        assert_eq!(c.clusters_at("parnamirim").len(), 1);
    }

    #[test]
    fn locations_deduped() {
        let c = fixture();
        let locs = c.locations();
        assert_eq!(locs, vec!["bristol".to_string(), "parnamirim".to_string()]);
    }
}
