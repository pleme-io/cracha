// AccessPolicy renderer — typed AccessPolicySpec → YAML CRD ready
// for `kubectl apply`. Used by `cracha render policy`.

use cracha_core::AccessPolicySpec;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum PolicyRenderError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

#[derive(Serialize)]
struct AccessPolicyCrd<'a> {
    #[serde(rename = "apiVersion")]
    api_version: &'static str,
    kind: &'static str,
    metadata: PolicyMeta<'a>,
    spec: &'a AccessPolicySpec,
}

#[derive(Serialize)]
struct PolicyMeta<'a> {
    name: &'a str,
    namespace: &'a str,
}

pub fn render(spec: &AccessPolicySpec, namespace: &str) -> Result<String, PolicyRenderError> {
    let crd = AccessPolicyCrd {
        api_version: "saguao.pleme.io/v1alpha1",
        kind: "AccessPolicy",
        metadata: PolicyMeta {
            name: &spec.name,
            namespace,
        },
        spec,
    };
    Ok(format!("---\n{}", serde_yaml::to_string(&crd)?))
}

pub fn render_to_file(
    spec: &AccessPolicySpec,
    namespace: &str,
    out: &Path,
) -> Result<(), PolicyRenderError> {
    let yaml = render(spec, namespace)?;
    std::fs::write(out, yaml)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cracha_core::{Grant, Verb};

    fn family() -> AccessPolicySpec {
        AccessPolicySpec {
            name: "family".into(),
            members: vec!["drzln".into(), "wife".into()],
            grants: vec![Grant {
                user: "drzln".into(),
                locations: vec!["*".into()],
                clusters: vec!["*".into()],
                services: vec!["*".into()],
                verbs: vec![Verb::All],
            }],
        }
    }

    #[test]
    fn renders_valid_yaml() {
        let s = render(&family(), "cracha").unwrap();
        assert!(s.starts_with("---"));
        assert!(s.contains("apiVersion: saguao.pleme.io/v1alpha1"));
        assert!(s.contains("kind: AccessPolicy"));
        assert!(s.contains("name: family"));
        let _: serde_yaml::Value = serde_yaml::from_str(&s).unwrap();
    }
}
