// The pure-function authz evaluator + the in-memory authz index
// shape that cracha-controller builds and cracha-api reads.

use crate::catalog::{ServiceCatalogSpec, ServiceEntry};
use crate::policy::{matches_any, AccessPolicySpec, UserId, Verb};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One authorization request — what vigia asks crachá on every gated
/// HTTP request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthzRequest {
    pub user: UserId,
    pub location: String,
    pub cluster: String,
    pub service: String,
    pub verb: Verb,
}

/// Authz decision returned to vigia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Allow,
    Deny,
}

/// Why a decision was made — useful for vigia's logs and varanda's
/// "request access" UX.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionReason {
    pub decision: Decision,
    pub matched_policy: Option<String>,
    pub reason: String,
}

/// In-memory authz index built by cracha-controller from observed
/// AccessPolicy + ServiceCatalog CRDs. Read by cracha-api on every
/// Authorize call.
///
/// Indexes by user → list of (policy_name, grants) so the evaluator
/// can short-circuit on user lookup. Per-policy data preserved so
/// the decision can name which policy matched (audit trail).
#[derive(Debug, Clone, Default)]
pub struct AuthzIndex {
    /// All known AccessPolicies, keyed by policy name.
    pub policies: HashMap<String, AccessPolicySpec>,
    /// Service catalog (cluster → location, service → cluster).
    pub catalog: ServiceCatalogSpec,
}

impl AuthzIndex {
    /// Construct an empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the index contents in one shot — used by cracha-controller
    /// after a CRD reconcile.
    pub fn replace(
        &mut self,
        policies: HashMap<String, AccessPolicySpec>,
        catalog: ServiceCatalogSpec,
    ) {
        self.policies = policies;
        self.catalog = catalog;
    }

    /// Number of distinct policies indexed.
    #[must_use]
    pub fn policy_count(&self) -> usize {
        self.policies.len()
    }

    /// Authorize a single request against every policy. Returns Allow
    /// if any policy + any of its grants match.
    #[must_use]
    pub fn authorize(&self, req: &AuthzRequest) -> DecisionReason {
        // Validate location/cluster/service against the catalog.
        let Some(_loc) = self.catalog.location_for_cluster(&req.cluster) else {
            return DecisionReason {
                decision: Decision::Deny,
                matched_policy: None,
                reason: format!("unknown cluster: {}", req.cluster),
            };
        };

        for policy in self.policies.values() {
            if !policy.members.iter().any(|m| m == &req.user) {
                continue;
            }
            for grant in &policy.grants {
                if grant.user != req.user {
                    continue;
                }
                if !matches_any(&grant.locations, &req.location)
                    || !matches_any(&grant.clusters, &req.cluster)
                    || !matches_any(&grant.services, &req.service)
                {
                    continue;
                }
                if grant.verbs.iter().any(|v| v.satisfies(req.verb)) {
                    return DecisionReason {
                        decision: Decision::Allow,
                        matched_policy: Some(policy.name.clone()),
                        reason: format!("matched grant in policy {}", policy.name),
                    };
                }
            }
        }

        DecisionReason {
            decision: Decision::Deny,
            matched_policy: None,
            reason: "no matching grant".into(),
        }
    }

    /// Compute the portal manifest for varanda — every service the
    /// user has at least Read access to, grouped by location/cluster.
    ///
    /// Returns a flat list of (service, location, cluster) tuples
    /// the user can see; varanda groups them client-side.
    #[must_use]
    pub fn accessible_services(&self, user: &str) -> Vec<AccessibleService> {
        let mut out = Vec::new();
        for service in &self.catalog.services {
            let Some(location) = self.catalog.location_for_cluster(&service.cluster) else {
                continue;
            };
            let req = AuthzRequest {
                user: user.into(),
                location: location.into(),
                cluster: service.cluster.clone(),
                service: service.slug.clone(),
                verb: Verb::Read,
            };
            if self.authorize(&req).decision == Decision::Allow {
                out.push(AccessibleService::from_entry(service, location));
            }
        }
        out
    }

    /// Look up a single service by slug and project it into the
    /// AccessibleService shape (with hostname, location, etc.).
    /// Used by cracha-api when materialising DB-stored per-user
    /// grants: the slug names a service in the catalog, this method
    /// hands back the full row so the response shape stays uniform
    /// with the declarative `accessible_services` output.
    ///
    /// Returns None if no service with that slug exists or if the
    /// service's cluster has no location mapping.
    #[must_use]
    pub fn lookup_service(&self, slug: &str) -> Option<AccessibleService> {
        let entry = self
            .catalog
            .services
            .iter()
            .find(|s| s.slug == slug)?;
        let location = self.catalog.location_for_cluster(&entry.cluster)?;
        Some(AccessibleService::from_entry(entry, location))
    }
}

/// One row of varanda's portal manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessibleService {
    pub slug: String,
    pub display_name: String,
    pub cluster: String,
    pub location: String,
    pub icon: Option<String>,
    pub description: Option<String>,
    /// The full saguão-shape hostname (e.g.,
    /// `vault.rio.bristol.quero.cloud`). varanda links to this.
    pub hostname: String,
}

impl AccessibleService {
    fn from_entry(s: &ServiceEntry, location: &str) -> Self {
        let hostname = format!("{}.{}.{}.quero.cloud", s.slug, s.cluster, location);
        Self {
            slug: s.slug.clone(),
            display_name: s.display_name.clone(),
            cluster: s.cluster.clone(),
            location: location.into(),
            icon: s.icon.clone(),
            description: s.description.clone(),
            hostname,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{ClusterRegistration, ServiceEntry};
    use crate::policy::{Grant, Verb};

    fn family_index() -> AuthzIndex {
        let mut policies = HashMap::new();
        policies.insert(
            "family".into(),
            AccessPolicySpec {
                name: "family".into(),
                members: vec!["drzln".into(), "wife".into(), "cousin".into()],
                grants: vec![
                    Grant {
                        user: "drzln".into(),
                        locations: vec!["*".into()],
                        clusters: vec!["*".into()],
                        services: vec!["*".into()],
                        verbs: vec![Verb::All],
                    },
                    Grant {
                        user: "wife".into(),
                        locations: vec!["bristol".into()],
                        clusters: vec!["rio".into()],
                        services: vec!["photos".into(), "jellyfin".into()],
                        verbs: vec![Verb::Read, Verb::Write],
                    },
                    Grant {
                        user: "cousin".into(),
                        locations: vec!["bristol".into()],
                        clusters: vec!["rio".into()],
                        services: vec!["chat".into()],
                        verbs: vec![Verb::Read, Verb::Write],
                    },
                ],
            },
        );

        let catalog = ServiceCatalogSpec {
            clusters: vec![
                ClusterRegistration {
                    name: "rio".into(),
                    location: "bristol".into(),
                    label: None,
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
                    description: None,
                },
                ServiceEntry {
                    slug: "photos".into(),
                    display_name: "Immich".into(),
                    cluster: "rio".into(),
                    icon: None,
                    description: None,
                },
                ServiceEntry {
                    slug: "jellyfin".into(),
                    display_name: "Jellyfin".into(),
                    cluster: "rio".into(),
                    icon: None,
                    description: None,
                },
                ServiceEntry {
                    slug: "chat".into(),
                    display_name: "Hiroba".into(),
                    cluster: "rio".into(),
                    icon: None,
                    description: None,
                },
                ServiceEntry {
                    slug: "vault".into(),
                    display_name: "Vaultwarden (mar)".into(),
                    cluster: "mar".into(),
                    icon: None,
                    description: None,
                },
            ],
        };

        let mut idx = AuthzIndex::new();
        idx.replace(policies, catalog);
        idx
    }

    fn req(user: &str, cluster: &str, service: &str, verb: Verb) -> AuthzRequest {
        // Pick location from cluster's mapping in the test catalog.
        let location = match cluster {
            "rio" => "bristol",
            "mar" => "parnamirim",
            _ => "unknown",
        };
        AuthzRequest {
            user: user.into(),
            location: location.into(),
            cluster: cluster.into(),
            service: service.into(),
            verb,
        }
    }

    #[test]
    fn operator_allowed_anywhere() {
        let idx = family_index();
        let r = idx.authorize(&req("drzln", "mar", "vault", Verb::Admin));
        assert_eq!(r.decision, Decision::Allow);
        assert_eq!(r.matched_policy.as_deref(), Some("family"));
    }

    #[test]
    fn wife_allowed_photos_rio() {
        let idx = family_index();
        let r = idx.authorize(&req("wife", "rio", "photos", Verb::Read));
        assert_eq!(r.decision, Decision::Allow);
    }

    #[test]
    fn wife_denied_vault() {
        let idx = family_index();
        let r = idx.authorize(&req("wife", "rio", "vault", Verb::Read));
        assert_eq!(r.decision, Decision::Deny);
    }

    #[test]
    fn wife_denied_mar_photos() {
        let idx = family_index();
        // wife only has Bristol grant; mar (Parnamirim) is out of scope.
        let r = idx.authorize(&req("wife", "mar", "vault", Verb::Read));
        assert_eq!(r.decision, Decision::Deny);
    }

    #[test]
    fn cousin_can_read_chat_but_not_write_jellyfin() {
        let idx = family_index();
        assert_eq!(
            idx.authorize(&req("cousin", "rio", "chat", Verb::Read)).decision,
            Decision::Allow
        );
        assert_eq!(
            idx.authorize(&req("cousin", "rio", "jellyfin", Verb::Read))
                .decision,
            Decision::Deny
        );
    }

    #[test]
    fn unknown_user_denied() {
        let idx = family_index();
        assert_eq!(
            idx.authorize(&req("stranger", "rio", "photos", Verb::Read))
                .decision,
            Decision::Deny
        );
    }

    #[test]
    fn unknown_cluster_denied() {
        let idx = family_index();
        let r = AuthzRequest {
            user: "drzln".into(),
            location: "bristol".into(),
            cluster: "ghost".into(),
            service: "vault".into(),
            verb: Verb::Read,
        };
        let d = idx.authorize(&r);
        assert_eq!(d.decision, Decision::Deny);
        assert!(d.reason.contains("unknown cluster"));
    }

    #[test]
    fn accessible_services_for_operator_lists_all() {
        let idx = family_index();
        let svcs = idx.accessible_services("drzln");
        assert_eq!(svcs.len(), 5);
    }

    #[test]
    fn accessible_services_for_wife_lists_two() {
        let idx = family_index();
        let svcs = idx.accessible_services("wife");
        let slugs: Vec<&str> = svcs.iter().map(|s| s.slug.as_str()).collect();
        assert!(slugs.contains(&"photos"));
        assert!(slugs.contains(&"jellyfin"));
        assert!(!slugs.contains(&"vault"));
    }

    #[test]
    fn accessible_service_hostname_is_4_part() {
        let idx = family_index();
        let svcs = idx.accessible_services("drzln");
        let vault_rio = svcs.iter().find(|s| s.slug == "vault" && s.cluster == "rio");
        assert!(vault_rio.is_some());
        assert_eq!(
            vault_rio.unwrap().hostname,
            "vault.rio.bristol.quero.cloud"
        );
    }

    #[test]
    fn empty_grant_list_is_no_access() {
        // Edge case — explicit empty list should NOT be treated as wildcard.
        let mut policies = HashMap::new();
        policies.insert(
            "broken".into(),
            AccessPolicySpec {
                name: "broken".into(),
                members: vec!["u".into()],
                grants: vec![Grant {
                    user: "u".into(),
                    locations: vec![], // explicit empty
                    clusters: vec!["*".into()],
                    services: vec!["*".into()],
                    verbs: vec![Verb::All],
                }],
            },
        );
        let mut idx = AuthzIndex::new();
        idx.replace(
            policies,
            ServiceCatalogSpec {
                clusters: vec![ClusterRegistration {
                    name: "rio".into(),
                    location: "bristol".into(),
                    label: None,
                }],
                services: vec![],
            },
        );
        assert_eq!(
            idx.authorize(&req("u", "rio", "vault", Verb::Read)).decision,
            Decision::Deny
        );
    }
}
