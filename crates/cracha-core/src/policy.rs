// AccessPolicy + AccessGroup CRDs and the typed grant model.

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A user identifier — typically the OIDC `sub` or email claim.
pub type UserId = String;

/// The five-verb lattice. Per-resource ACL is intentionally out of
/// scope for crachá (lives inside the application).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Verb {
    /// View-only (HTTP GET).
    Read,
    /// Create / update (HTTP POST / PUT / PATCH).
    Write,
    /// Remove (HTTP DELETE).
    Delete,
    /// Configure the service itself (admin endpoints).
    Admin,
    /// All of the above.
    #[serde(rename = "*", alias = "all")]
    All,
}

impl Verb {
    /// Map an HTTP method to the verb the request asserts.
    #[must_use]
    pub fn for_http_method(method: &str) -> Self {
        match method.to_ascii_uppercase().as_str() {
            "GET" | "HEAD" | "OPTIONS" => Verb::Read,
            "POST" | "PUT" | "PATCH" => Verb::Write,
            "DELETE" => Verb::Delete,
            // Custom methods (e.g., WebDAV) default to write — fail-closed.
            _ => Verb::Write,
        }
    }

    /// Does a granted verb satisfy a requested verb?
    #[must_use]
    pub fn satisfies(self, requested: Verb) -> bool {
        match self {
            Verb::All => true,
            v => v == requested,
        }
    }
}

/// One grant — "this user, in these locations / clusters / services,
/// with these verbs."
///
/// Star-wildcards are allowed for any field except `user`. Empty
/// lists are treated as "no access" (NOT as wildcards) to fail-closed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Grant {
    /// User this grant applies to.
    pub user: UserId,

    /// Locations in scope; `["*"]` means all known locations.
    #[serde(default = "wildcard")]
    pub locations: Vec<String>,

    /// Clusters in scope; `["*"]` means all known clusters within
    /// the matched locations.
    #[serde(default = "wildcard")]
    pub clusters: Vec<String>,

    /// Services in scope; `["*"]` means all services on the matched
    /// clusters.
    #[serde(default = "wildcard")]
    pub services: Vec<String>,

    /// Verbs in scope.
    #[serde(default = "all_verbs")]
    pub verbs: Vec<Verb>,
}

/// Spec field of the AccessPolicy CRD — the typed authorization
/// policy for one named group (e.g., "family", "operators", "guests").
#[derive(
    Debug, Clone, Serialize, Deserialize, JsonSchema, CustomResource, PartialEq, Eq,
)]
#[kube(
    group = "saguao.pleme.io",
    version = "v1alpha1",
    kind = "AccessPolicy",
    plural = "accesspolicies",
    shortname = "apol",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct AccessPolicySpec {
    /// Logical name of the policy (e.g., "family"). Must be unique
    /// within the namespace.
    pub name: String,

    /// User identifiers (email or sub-claim) included in this policy.
    /// A user not listed here is denied even if a grant references
    /// them — this is the membership gate.
    pub members: Vec<UserId>,

    /// What each member is allowed to do.
    pub grants: Vec<Grant>,
}

/// Spec field of the AccessGroup CRD — a fleet-wide named group
/// referenced by AccessPolicies. Optional; AccessPolicy can list
/// members inline.
#[derive(
    Debug, Clone, Serialize, Deserialize, JsonSchema, CustomResource, PartialEq, Eq,
)]
#[kube(
    group = "saguao.pleme.io",
    version = "v1alpha1",
    kind = "AccessGroup",
    plural = "accessgroups",
    shortname = "agrp",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct AccessGroupSpec {
    /// Logical name of the group.
    pub name: String,
    /// Member emails / sub-claims.
    pub members: Vec<UserId>,
}

/// Errors raised by AccessPolicy validation or evaluation.
#[derive(Debug, thiserror::Error)]
pub enum CrachaError {
    #[error("user not in policy members: {0}")]
    UserNotMember(UserId),
    #[error("unknown cluster: {0}")]
    UnknownCluster(String),
    #[error("unknown location: {0}")]
    UnknownLocation(String),
    #[error("invalid policy: {0}")]
    InvalidPolicy(String),
}

fn wildcard() -> Vec<String> {
    vec!["*".into()]
}

fn all_verbs() -> Vec<Verb> {
    vec![Verb::All]
}

/// Pattern matcher: `"*"` matches anything; otherwise exact match.
#[must_use]
pub fn matches_any(patterns: &[String], value: &str) -> bool {
    if patterns.is_empty() {
        return false; // explicit empty list = no access (fail-closed)
    }
    patterns.iter().any(|p| p == "*" || p == value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verb_satisfies_all() {
        assert!(Verb::All.satisfies(Verb::Read));
        assert!(Verb::All.satisfies(Verb::Admin));
    }

    #[test]
    fn verb_satisfies_self() {
        assert!(Verb::Read.satisfies(Verb::Read));
        assert!(!Verb::Read.satisfies(Verb::Write));
    }

    #[test]
    fn http_method_mapping() {
        assert_eq!(Verb::for_http_method("get"), Verb::Read);
        assert_eq!(Verb::for_http_method("POST"), Verb::Write);
        assert_eq!(Verb::for_http_method("DELETE"), Verb::Delete);
        assert_eq!(Verb::for_http_method("MKCOL"), Verb::Write); // unknown → fail-closed to write
    }

    #[test]
    fn matches_any_wildcard() {
        assert!(matches_any(&["*".into()], "anything"));
        assert!(matches_any(&["a".into(), "b".into()], "a"));
        assert!(!matches_any(&["a".into(), "b".into()], "c"));
        assert!(!matches_any(&[], "anything")); // empty = no access
    }

    #[test]
    fn grant_serializes_round_trip() {
        let g = Grant {
            user: "drzln".into(),
            locations: vec!["bristol".into()],
            clusters: vec!["rio".into()],
            services: vec!["vault".into()],
            verbs: vec![Verb::Read, Verb::Write],
        };
        let s = serde_json::to_string(&g).unwrap();
        let g2: Grant = serde_json::from_str(&s).unwrap();
        assert_eq!(g, g2);
    }

    #[test]
    fn star_verb_serializes() {
        assert_eq!(serde_json::to_string(&Verb::All).unwrap(), "\"*\"");
        let v: Verb = serde_json::from_str("\"*\"").unwrap();
        assert_eq!(v, Verb::All);
    }
}
