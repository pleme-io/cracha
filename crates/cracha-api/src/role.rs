// Role derivation — pure, no I/O. The admin allowlist is a
// git-declared HashSet at startup; runtime promotion is intentionally
// not supported (edit shikumi instead).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    User,
}

/// Compute the role from a user's email + the declarative allowlist.
/// Comparison is case-insensitive — both sides .lowercase() before
/// matching so `Drzln@ProtonMail.COM` and `drzln@protonmail.com` are
/// equivalent.
#[must_use]
pub fn compute_role(email: &str, admins: &HashSet<String>) -> Role {
    if admins.contains(&email.to_lowercase()) {
        Role::Admin
    } else {
        Role::User
    }
}
