//! Load and validate the committed `authorized-users.toml` allowlist.
//!
//! Sign-in is gated by membership in that file, keyed case-insensitively
//! by email. Adding or removing a user is a PR. Duplicates and (in
//! production) an empty list are boot errors.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

use crate::config::Environment;

#[derive(Debug, Clone)]
pub struct AuthorizedUser {
    pub admin: bool,
}

/// Case-insensitive map keyed by lowercased email → user record.
#[derive(Debug, Clone, Default)]
pub struct AccessList {
    by_email: BTreeMap<String, AuthorizedUser>,
}

impl AccessList {
    pub fn load_from_file(path: impl AsRef<Path>, env: Environment) -> Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;

        #[derive(Deserialize)]
        struct File {
            #[serde(default)]
            users: Vec<Entry>,
        }
        #[derive(Deserialize)]
        struct Entry {
            email: String,
            #[serde(default)]
            admin: bool,
        }

        let file: File = toml::from_str(&text)
            .with_context(|| format!("parsing {}", path.display()))?;

        let mut by_email: BTreeMap<String, AuthorizedUser> = BTreeMap::new();
        for entry in file.users {
            let email = entry.email.trim().to_ascii_lowercase();
            if email.is_empty() {
                return Err(anyhow!("empty email in {}", path.display()));
            }
            if by_email.contains_key(&email) {
                return Err(anyhow!(
                    "duplicate email `{email}` in {}",
                    path.display()
                ));
            }
            by_email.insert(
                email,
                AuthorizedUser {
                    admin: entry.admin,
                },
            );
        }

        if env == Environment::Production && by_email.is_empty() {
            return Err(anyhow!(
                "authorized-users.toml has zero entries in production; nobody could sign in"
            ));
        }

        Ok(AccessList { by_email })
    }

    /// Case-insensitive lookup. Returns the matched entry or None.
    pub fn check(&self, email: &str) -> Option<&AuthorizedUser> {
        self.by_email.get(&email.trim().to_ascii_lowercase())
    }

    pub fn len(&self) -> usize {
        self.by_email.len()
    }
}
