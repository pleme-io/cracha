// Per-cluster artifact renderer.
//
// One `Cluster` declaration → 4 emitted files:
//   1. nix-fleet-domains-<cluster>.nix.fragment — appendable to
//      pleme-io/nix/lib/pleme-fleet.nix's `locations = { … };` map.
//   2. vigia-<cluster>-helmrelease.yaml — drop into
//      pleme-io/k8s/clusters/<cluster>/infrastructure/vigia/release.yaml.
//   3. pangea-cloudflare-pleme-<cluster>-additions.yaml — fragment
//      to merge into pleme-io/pangea-architectures/workspaces/cloudflare-pleme/domains/quero.cloud.yaml
//      (for the cluster portal hostname + tunnel ingress).
//   4. cracha-cluster-registration-<cluster>.yaml — appendable to the
//      ServiceCatalog ConfigMap (or applied as a snippet into the
//      cluster registry CRD).

use cracha_core::{Cluster, ClusterRole};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

/// Render every per-cluster artifact for `cluster` into `out_dir`.
pub fn render_all(cluster: &Cluster, out_dir: &Path) -> Result<Vec<String>, RenderError> {
    let mut written = Vec::new();
    let name = &cluster.name;

    let nix = render_nix_fragment(cluster);
    let nix_name = format!("nix-fleet-domains-{name}.nix.fragment");
    super::write_artifact(out_dir, &nix_name, &nix)?;
    written.push(nix_name);

    let vigia = render_vigia_helmrelease(cluster)?;
    let vigia_name = format!("vigia-{name}-helmrelease.yaml");
    super::write_artifact(out_dir, &vigia_name, &vigia)?;
    written.push(vigia_name);

    let pangea = render_pangea_additions(cluster);
    let pangea_name = format!("pangea-cloudflare-pleme-{name}-additions.yaml");
    super::write_artifact(out_dir, &pangea_name, &pangea)?;
    written.push(pangea_name);

    let registration = render_cluster_registration(cluster)?;
    let registration_name = format!("cracha-cluster-registration-{name}.yaml");
    super::write_artifact(out_dir, &registration_name, &registration)?;
    written.push(registration_name);

    Ok(written)
}

/// Emit a single line for nix/lib/pleme-fleet.nix's `locations`
/// map. Operator merges this into the existing map.
fn render_nix_fragment(cluster: &Cluster) -> String {
    let mut s = String::new();
    s.push_str("# Append to nix/lib/pleme-fleet.nix's `locations = { … };` map:\n");
    if let Some(label) = &cluster.label {
        s.push_str(&format!("# {label}\n"));
    }
    if let Some(country) = &cluster.country {
        s.push_str(&format!("# {country}\n"));
    }
    s.push_str(&format!("{} = \"{}\";\n", cluster.name, cluster.location));
    if let Some(user) = &cluster.ssh_user {
        s.push_str(&format!("# Append to `sshUsers`:\n# {} = \"{}\";\n", cluster.name, user));
    }
    s
}

/// Emit the lareira-vigia HelmRelease YAML for this cluster.
fn render_vigia_helmrelease(cluster: &Cluster) -> Result<String, RenderError> {
    #[derive(Serialize)]
    struct Hr {
        #[serde(rename = "apiVersion")]
        api_version: &'static str,
        kind: &'static str,
        metadata: HrMeta,
        spec: HrSpec,
    }
    #[derive(Serialize)]
    struct HrMeta {
        name: String,
        namespace: String,
    }
    #[derive(Serialize)]
    struct HrSpec {
        interval: &'static str,
        chart: HrChart,
        install: HrRem,
        upgrade: HrRem,
        values: HrValues,
    }
    #[derive(Serialize)]
    struct HrChart {
        spec: HrChartSpec,
    }
    #[derive(Serialize)]
    struct HrChartSpec {
        chart: &'static str,
        version: &'static str,
        #[serde(rename = "sourceRef")]
        source_ref: HrSourceRef,
    }
    #[derive(Serialize)]
    struct HrSourceRef {
        kind: &'static str,
        name: &'static str,
        namespace: &'static str,
    }
    #[derive(Serialize)]
    struct HrRem {
        remediation: HrRemediation,
    }
    #[derive(Serialize)]
    struct HrRemediation {
        retries: u32,
    }
    #[derive(Serialize)]
    struct HrValues {
        cluster: String,
        location: String,
    }

    let hr = Hr {
        api_version: "helm.toolkit.fluxcd.io/v2",
        kind: "HelmRelease",
        metadata: HrMeta {
            name: "lareira-vigia".into(),
            namespace: "vigia".into(),
        },
        spec: HrSpec {
            interval: "30m",
            chart: HrChart {
                spec: HrChartSpec {
                    chart: "lareira-vigia",
                    version: "0.1.x",
                    source_ref: HrSourceRef {
                        kind: "HelmRepository",
                        name: "pleme-charts",
                        namespace: "flux-system",
                    },
                },
            },
            install: HrRem { remediation: HrRemediation { retries: 3 } },
            upgrade: HrRem { remediation: HrRemediation { retries: 3 } },
            values: HrValues {
                cluster: cluster.name.clone(),
                location: cluster.location.clone(),
            },
        },
    };

    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("# Drop into k8s/clusters/<cluster>/infrastructure/vigia/release.yaml\n");
    out.push_str(&serde_yaml::to_string(&hr)?);
    Ok(out)
}

/// Emit Pangea fragment additions to the cloudflare-pleme workspace's
/// quero.cloud.yaml — the cluster portal hostname's tunnel CNAME.
fn render_pangea_additions(cluster: &Cluster) -> String {
    let cluster_portal = cluster.cluster_portal_hostname();
    let location_portal = format!("{}.quero.cloud", cluster.location);

    let mut s = String::new();
    s.push_str("# Merge into pangea-architectures/workspaces/cloudflare-pleme/domains/quero.cloud.yaml\n");
    s.push_str("# under `dns_records:` (and ensure the Pages project's custom_domains list includes both names).\n");
    s.push_str("dns_records:\n");
    s.push_str(&format!(
        "  - {{ name: {cluster_portal:?}, type: CNAME, content: \"varanda.pages.dev\", proxied: true, ttl: 1 }}\n"
    ));
    s.push_str(&format!(
        "  - {{ name: {location_portal:?}, type: CNAME, content: \"varanda.pages.dev\", proxied: true, ttl: 1 }}\n"
    ));
    s.push_str("# Cluster's lareira HelmReleases will add their own per-app tunnel CNAMEs\n");
    s.push_str(&format!(
        "# under the pattern <app>.{}.{}.quero.cloud.\n",
        cluster.name, cluster.location
    ));
    s
}

/// Emit a cracha cluster-registration YAML — typically merged into a
/// ServiceCatalog CRD on the control-plane cluster.
fn render_cluster_registration(cluster: &Cluster) -> Result<String, RenderError> {
    #[derive(Serialize)]
    struct Reg {
        name: String,
        location: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    }
    let r = Reg {
        name: cluster.name.clone(),
        location: cluster.location.clone(),
        label: cluster.label.clone(),
    };
    let mut s = String::new();
    s.push_str("# Append to the ServiceCatalog spec.clusters list on the control-plane cluster\n");
    s.push_str(&format!(
        "# (typically: pleme-io/k8s/clusters/{}/infrastructure/cracha/service-catalog.yaml).\n",
        match cluster.role {
            ClusterRole::ControlPlane | ClusterRole::Hybrid => cluster.name.as_str(),
            ClusterRole::Consumer => "rio",
        }
    ));
    s.push_str("clusters:\n");
    s.push_str(&format!("  - {}\n", serde_yaml::to_string(&r)?.replace("\n", "\n    ").trim_end()));
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cracha_core::SaguaoComponents;

    fn mar() -> Cluster {
        Cluster {
            name: "mar".into(),
            location: "parnamirim".into(),
            label: Some("Parnamirim home edge".into()),
            country: Some("RN, Brazil".into()),
            role: ClusterRole::Consumer,
            saguao: SaguaoComponents::default(),
            ssh_user: Some("luis".into()),
        }
    }

    #[test]
    fn nix_fragment_includes_name_and_location() {
        let s = render_nix_fragment(&mar());
        assert!(s.contains("mar = \"parnamirim\";"));
        assert!(s.contains("Parnamirim home edge"));
    }

    #[test]
    fn vigia_helmrelease_round_trips_through_yaml() {
        let s = render_vigia_helmrelease(&mar()).unwrap();
        assert!(s.contains("name: lareira-vigia"));
        assert!(s.contains("cluster: mar"));
        assert!(s.contains("location: parnamirim"));
        // Sanity: parses as YAML.
        let _: serde_yaml::Value = serde_yaml::from_str(&s).unwrap();
    }

    #[test]
    fn pangea_fragment_emits_4_part_aware_routes() {
        let s = render_pangea_additions(&mar());
        assert!(s.contains("mar.parnamirim.quero.cloud"));
        assert!(s.contains("parnamirim.quero.cloud"));
    }

    #[test]
    fn cluster_registration_includes_location() {
        let s = render_cluster_registration(&mar()).unwrap();
        assert!(s.contains("name: mar"));
        assert!(s.contains("location: parnamirim"));
    }
}
