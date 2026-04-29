// Cluster + Fleet typed forms — the typescape primitives that close the
// "adding a cluster requires N edits across N repos" seam.
//
// One `(defcluster …)` form is the single source of truth for a
// cluster's saguão integration. The renderer (cracha-cli) takes the
// form and emits every per-cluster artifact mechanically.
//
// `(deffleet …)` is the umbrella composition — the typed answer to
// "what's in this fleet?".

use serde::{Deserialize, Serialize};

#[cfg(feature = "tatara-lisp")]
use tatara_lisp_derive::TataraDomain;

/// What role this cluster plays in the saguão control plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ClusterRole {
    /// Just runs vigia + varanda; consumes fleet's passaporte + crachá.
    /// Default for every cluster except the one hosting the control plane.
    #[default]
    Consumer,
    /// Hosts passaporte + crachá in addition to vigia + varanda.
    /// Today: rio. Tomorrow: a tiny dedicated control-plane cluster.
    ControlPlane,
    /// Hosts a fallback/HA replica of passaporte + crachá. Phase-3 of
    /// the IdP placement trajectory in SAGUAO §VI.
    Hybrid,
}

/// Which saguão components are deployed to this cluster.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaguaoComponents {
    /// Per-cluster forward-auth — true for every gated cluster.
    #[serde(default = "default_true")]
    pub vigia: bool,
    /// Cluster-view portal at `<cluster>.<location>.quero.cloud`.
    #[serde(default = "default_true")]
    pub varanda: bool,
    /// Hosts passaporte (control plane). None = consumes fleet's.
    #[serde(default)]
    pub passaporte: bool,
    /// Hosts crachá (control plane). None = consumes fleet's.
    #[serde(default)]
    pub cracha: bool,
}

impl Default for SaguaoComponents {
    fn default() -> Self {
        Self {
            vigia: true,
            varanda: true,
            passaporte: false,
            cracha: false,
        }
    }
}

const fn default_true() -> bool {
    true
}

/// One cluster in the fleet. The single source of truth.
///
/// Authored as `(defcluster :name mar :location parnamirim
/// :role consumer :saguao {:vigia true :varanda true})`. The renderer
/// emits Nix entries, vigia HelmRelease, Pangea tunnel CNAMEs,
/// Cloudflare Pages custom-domains, and crachá ClusterRegistration
/// from this single declaration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "tatara-lisp", derive(TataraDomain))]
#[cfg_attr(feature = "tatara-lisp", tatara(keyword = "defcluster"))]
pub struct Cluster {
    /// Cluster name (e.g., "rio", "mar"). Lowercase, 3-letter
    /// convention but not enforced.
    pub name: String,

    /// Location (e.g., "bristol", "parnamirim").
    pub location: String,

    /// Optional human-readable label.
    #[serde(default)]
    pub label: Option<String>,

    /// Optional country/region (for varanda's location header).
    #[serde(default)]
    pub country: Option<String>,

    /// What role this cluster plays. Default: Consumer.
    #[serde(default = "default_role")]
    pub role: ClusterRole,

    /// Which saguão components run here. Default: vigia + varanda only.
    #[serde(default)]
    pub saguao: SaguaoComponents,

    /// SSH user for ad-hoc operator access. Optional — pleme-fleet.nix
    /// has its own defaults.
    #[serde(default)]
    pub ssh_user: Option<String>,
}

const fn default_role() -> ClusterRole {
    ClusterRole::Consumer
}

impl Cluster {
    /// Canonical hostname for a workload service on this cluster:
    /// `<app>.<cluster>.<location>.quero.cloud`.
    #[must_use]
    pub fn service_hostname(&self, app: &str) -> String {
        format!("{app}.{}.{}.quero.cloud", self.name, self.location)
    }

    /// Cluster portal hostname — what varanda renders at the cluster view.
    #[must_use]
    pub fn cluster_portal_hostname(&self) -> String {
        format!("{}.{}.quero.cloud", self.name, self.location)
    }

    /// Does this cluster host the fleet's control plane?
    #[must_use]
    pub fn is_control_plane(&self) -> bool {
        matches!(self.role, ClusterRole::ControlPlane | ClusterRole::Hybrid)
    }
}

/// One reference to where a fleet primitive (passaporte / crachá) lives.
/// Today this is just a hostname — but the typed wrapper means we can
/// extend it to (cluster, namespace, hostname) when HA replication
/// lands without breaking callers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FleetEndpoint {
    /// Public hostname (e.g., "auth.quero.cloud").
    pub host: String,
    /// Which cluster physically hosts it (for documentation + monitoring).
    pub on_cluster: String,
}

/// The umbrella fleet declaration — the typed answer to "what's in
/// this fleet?". One per pleme-io homelab installation.
///
/// Authored as:
///   (deffleet :name pleme
///             :clusters [(defcluster :name rio :location bristol :role control-plane …)
///                        (defcluster :name mar :location parnamirim …)]
///             :passaporte (:host auth.quero.cloud :on-cluster rio)
///             :cracha (:host cracha.quero.cloud :on-cluster rio))
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "tatara-lisp", derive(TataraDomain))]
#[cfg_attr(feature = "tatara-lisp", tatara(keyword = "deffleet"))]
pub struct Fleet {
    /// Fleet name (e.g., "pleme").
    pub name: String,

    /// All clusters in the fleet.
    pub clusters: Vec<Cluster>,

    /// Where passaporte (the IdP) lives.
    pub passaporte: FleetEndpoint,

    /// Where crachá (the authz API) lives.
    pub cracha: FleetEndpoint,

    /// Apex domain (e.g., "quero.cloud").
    #[serde(default = "default_tld")]
    pub tld: String,
}

fn default_tld() -> String {
    "quero.cloud".into()
}

impl Fleet {
    /// All distinct locations referenced by any cluster, sorted +
    /// deduped.
    #[must_use]
    pub fn locations(&self) -> Vec<String> {
        let mut locs: Vec<String> =
            self.clusters.iter().map(|c| c.location.clone()).collect();
        locs.sort();
        locs.dedup();
        locs
    }

    /// All clusters at a given location.
    #[must_use]
    pub fn clusters_at(&self, location: &str) -> Vec<&Cluster> {
        self.clusters.iter().filter(|c| c.location == location).collect()
    }

    /// Look up a cluster by name.
    #[must_use]
    pub fn cluster(&self, name: &str) -> Option<&Cluster> {
        self.clusters.iter().find(|c| c.name == name)
    }

    /// The single cluster currently hosting the fleet's control plane.
    /// Returns the first ControlPlane (or Hybrid) cluster found, or
    /// the cluster the passaporte endpoint names if no role is set.
    #[must_use]
    pub fn control_plane_cluster(&self) -> Option<&Cluster> {
        self.clusters
            .iter()
            .find(|c| c.is_control_plane())
            .or_else(|| self.cluster(&self.passaporte.on_cluster))
    }

    /// Validation: every endpoint's `on_cluster` must reference a
    /// declared cluster; the control-plane cluster must have its
    /// passaporte/cracha SaguaoComponents flags true.
    ///
    /// Returns a vec of validation errors (empty = ok).
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut errs = Vec::new();
        if self.cluster(&self.passaporte.on_cluster).is_none() {
            errs.push(format!(
                "passaporte.on_cluster '{}' not in clusters list",
                self.passaporte.on_cluster
            ));
        }
        if self.cluster(&self.cracha.on_cluster).is_none() {
            errs.push(format!(
                "cracha.on_cluster '{}' not in clusters list",
                self.cracha.on_cluster
            ));
        }
        if let Some(cp) = self.cluster(&self.passaporte.on_cluster) {
            if !cp.saguao.passaporte {
                errs.push(format!(
                    "cluster '{}' hosts passaporte but its saguao.passaporte flag is false",
                    cp.name
                ));
            }
        }
        if let Some(cp) = self.cluster(&self.cracha.on_cluster) {
            if !cp.saguao.cracha {
                errs.push(format!(
                    "cluster '{}' hosts cracha but its saguao.cracha flag is false",
                    cp.name
                ));
            }
        }
        errs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rio_cluster() -> Cluster {
        Cluster {
            name: "rio".into(),
            location: "bristol".into(),
            label: Some("Bristol home edge".into()),
            country: Some("TN, USA".into()),
            role: ClusterRole::ControlPlane,
            saguao: SaguaoComponents {
                vigia: true,
                varanda: true,
                passaporte: true,
                cracha: true,
            },
            ssh_user: Some("luis".into()),
        }
    }

    fn mar_cluster() -> Cluster {
        Cluster {
            name: "mar".into(),
            location: "parnamirim".into(),
            label: None,
            country: Some("RN, Brazil".into()),
            role: ClusterRole::Consumer,
            saguao: SaguaoComponents::default(),
            ssh_user: None,
        }
    }

    fn fleet_fixture() -> Fleet {
        Fleet {
            name: "pleme".into(),
            clusters: vec![rio_cluster(), mar_cluster()],
            passaporte: FleetEndpoint {
                host: "auth.quero.cloud".into(),
                on_cluster: "rio".into(),
            },
            cracha: FleetEndpoint {
                host: "cracha.quero.cloud".into(),
                on_cluster: "rio".into(),
            },
            tld: "quero.cloud".into(),
        }
    }

    #[test]
    fn cluster_service_hostname_is_4_part() {
        let c = rio_cluster();
        assert_eq!(
            c.service_hostname("vault"),
            "vault.rio.bristol.quero.cloud"
        );
    }

    #[test]
    fn cluster_portal_hostname_is_3_part() {
        let c = mar_cluster();
        assert_eq!(c.cluster_portal_hostname(), "mar.parnamirim.quero.cloud");
    }

    #[test]
    fn control_plane_detection() {
        assert!(rio_cluster().is_control_plane());
        assert!(!mar_cluster().is_control_plane());
    }

    #[test]
    fn fleet_locations_deduped() {
        let f = fleet_fixture();
        let locs = f.locations();
        assert_eq!(locs, vec!["bristol".to_string(), "parnamirim".to_string()]);
    }

    #[test]
    fn fleet_clusters_at_location() {
        let f = fleet_fixture();
        assert_eq!(f.clusters_at("bristol").len(), 1);
        assert_eq!(f.clusters_at("parnamirim").len(), 1);
        assert_eq!(f.clusters_at("nyc").len(), 0);
    }

    #[test]
    fn fleet_control_plane_finds_rio() {
        let f = fleet_fixture();
        assert_eq!(f.control_plane_cluster().map(|c| c.name.clone()), Some("rio".into()));
    }

    #[test]
    fn fleet_validates_clean() {
        let f = fleet_fixture();
        assert!(f.validate().is_empty());
    }

    #[test]
    fn fleet_validation_catches_dangling_passaporte_ref() {
        let mut f = fleet_fixture();
        f.passaporte.on_cluster = "ghost".into();
        let errs = f.validate();
        assert!(errs.iter().any(|e| e.contains("passaporte.on_cluster")));
    }

    #[test]
    fn fleet_validation_catches_passaporte_flag_mismatch() {
        let mut f = fleet_fixture();
        f.clusters[0].saguao.passaporte = false; // rio claims to host passaporte but flag false
        let errs = f.validate();
        assert!(errs.iter().any(|e| e.contains("hosts passaporte")));
    }

    #[test]
    fn cluster_round_trips_serde() {
        let c = rio_cluster();
        let s = serde_json::to_string(&c).unwrap();
        let c2: Cluster = serde_json::from_str(&s).unwrap();
        assert_eq!(c, c2);
    }

    #[test]
    fn fleet_round_trips_serde() {
        let f = fleet_fixture();
        let s = serde_json::to_string(&f).unwrap();
        let f2: Fleet = serde_json::from_str(&s).unwrap();
        assert_eq!(f, f2);
    }

    #[test]
    fn cluster_role_serializes_kebab_case() {
        assert_eq!(serde_json::to_string(&ClusterRole::ControlPlane).unwrap(), "\"control-plane\"");
        let r: ClusterRole = serde_json::from_str("\"consumer\"").unwrap();
        assert_eq!(r, ClusterRole::Consumer);
    }
}
