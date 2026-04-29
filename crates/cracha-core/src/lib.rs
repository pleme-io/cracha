// crachá-core — typed AccessPolicy IR for the saguão fleet.
//
// Pure types + serde + validation + evaluator. No I/O. The single
// source-of-truth shape for "who can see what" across the fleet.
//
// Author surface: (defcrachá …) tatara-lisp form (registered via
// TataraDomain in a downstream integration crate).
//
// Consumer surfaces:
//   - `AccessPolicy` CRD reconciled by cracha-controller
//   - `AccessGroup` CRD listing fleet-wide groups
//   - `ServiceCatalog` CRD enumerating known services per cluster
//   - `evaluate()` — pure decision function used by cracha-api

#![allow(clippy::module_name_repetitions)]

pub mod catalog;
pub mod evaluator;
pub mod policy;

pub use catalog::{ClusterRegistration, ServiceCatalog, ServiceCatalogSpec, ServiceEntry};
pub use evaluator::{AccessibleService, AuthzIndex, AuthzRequest, Decision, DecisionReason};
pub use policy::{
    AccessGroup, AccessGroupSpec, AccessPolicy, AccessPolicySpec, CrachaError, Grant, UserId, Verb,
};

/// Register crachá's typed Lisp surfaces with the global tatara-lisp
/// dispatcher. Call once at process start in cracha-controller /
/// cracha-api so `(defcrachá …)` and `(defaccessgroup …)` forms parse
/// against the typed IR.
#[cfg(feature = "tatara-lisp")]
pub fn register_lisp_surfaces() {
    tatara_lisp::domain::register::<AccessPolicySpec>();
    tatara_lisp::domain::register::<AccessGroupSpec>();
}
