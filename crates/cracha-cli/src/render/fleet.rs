// Fleet-level renderer — invokes the per-cluster renderer for every
// cluster in the Fleet, plus emits one fleet-summary file naming
// passaporte + crachá endpoints.

use super::cluster;
use cracha_core::Fleet;
use std::path::Path;

pub fn render_all(fleet: &Fleet, out_dir: &Path) -> Result<Vec<String>, cluster::RenderError> {
    // Validate the fleet first; surface errors before writing anything.
    let errs = fleet.validate();
    if !errs.is_empty() {
        eprintln!("fleet validation warnings:");
        for e in errs {
            eprintln!("  - {e}");
        }
    }

    let mut all_written = Vec::new();
    for c in &fleet.clusters {
        let cluster_out = out_dir.join(&c.name);
        let written = cluster::render_all(c, &cluster_out)?;
        for f in written {
            all_written.push(format!("{}/{}", c.name, f));
        }
    }

    let summary = render_summary(fleet);
    super::write_artifact(out_dir, "fleet-summary.md", &summary)?;
    all_written.push("fleet-summary.md".into());

    Ok(all_written)
}

fn render_summary(fleet: &Fleet) -> String {
    let mut s = String::new();
    s.push_str(&format!("# {} fleet — saguão summary\n\n", fleet.name));
    s.push_str(&format!("- TLD: `{}`\n", fleet.tld));
    s.push_str(&format!(
        "- passaporte: `{}` (hosted on `{}`)\n",
        fleet.passaporte.host, fleet.passaporte.on_cluster
    ));
    s.push_str(&format!(
        "- crachá: `{}` (hosted on `{}`)\n",
        fleet.cracha.host, fleet.cracha.on_cluster
    ));

    s.push_str("\n## Clusters\n\n");
    s.push_str("| Cluster | Location | Role | vigia | varanda | passaporte | crachá |\n");
    s.push_str("|---|---|---|---|---|---|---|\n");
    for c in &fleet.clusters {
        s.push_str(&format!(
            "| `{}` | `{}` | {:?} | {} | {} | {} | {} |\n",
            c.name,
            c.location,
            c.role,
            yn(c.saguao.vigia),
            yn(c.saguao.varanda),
            yn(c.saguao.passaporte),
            yn(c.saguao.cracha),
        ));
    }

    s.push_str("\n## Locations\n\n");
    for loc in fleet.locations() {
        s.push_str(&format!("- `{loc}` ({} clusters)\n", fleet.clusters_at(&loc).len()));
    }

    s
}

fn yn(b: bool) -> &'static str {
    if b { "✓" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cracha_core::{Cluster, ClusterRole, FleetEndpoint, SaguaoComponents};

    fn fleet() -> Fleet {
        Fleet {
            name: "pleme".into(),
            clusters: vec![
                Cluster {
                    name: "rio".into(),
                    location: "bristol".into(),
                    label: None,
                    country: None,
                    role: ClusterRole::ControlPlane,
                    saguao: SaguaoComponents {
                        vigia: true,
                        varanda: true,
                        passaporte: true,
                        cracha: true,
                    },
                    ssh_user: None,
                },
                Cluster {
                    name: "mar".into(),
                    location: "parnamirim".into(),
                    label: None,
                    country: None,
                    role: ClusterRole::Consumer,
                    saguao: SaguaoComponents::default(),
                    ssh_user: None,
                },
            ],
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
    fn summary_lists_every_cluster() {
        let s = render_summary(&fleet());
        assert!(s.contains("`rio`"));
        assert!(s.contains("`mar`"));
        assert!(s.contains("auth.quero.cloud"));
        assert!(s.contains("cracha.quero.cloud"));
    }
}
