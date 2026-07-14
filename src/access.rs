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

#[cfg(test)]
mod tests {
    // Covers functional / edge / negative / error dimensions.
    // state-transition N/A: AccessList is load-once, read-only — no in-place
    // mutations exist and reloads happen only at boot.

    use super::*;
    use tempfile::TempDir;

    /// Write `contents` to a temp `authorized-users.toml` and return the tempdir + path.
    /// Tempdir must be kept alive for the file to exist.
    fn write_allow_file(contents: &str) -> (TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("authorized-users.toml");
        std::fs::write(&path, contents).expect("write test file");
        (tmp, path)
    }

    #[test]
    fn load_from_file_parses_one_admin_user() {
        let (_tmp, path) = write_allow_file(
            "[[users]]\nemail = \"alice@example.com\"\nadmin = true\n",
        );

        let list = AccessList::load_from_file(&path, Environment::Development).unwrap();

        assert_eq!(list.len(), 1);
        let entry = list.check("alice@example.com").expect("match");
        assert!(entry.admin);
    }

    #[test]
    fn load_from_file_defaults_admin_to_false_when_omitted() {
        let (_tmp, path) = write_allow_file("[[users]]\nemail = \"bob@example.com\"\n");

        let list = AccessList::load_from_file(&path, Environment::Development).unwrap();

        let entry = list.check("bob@example.com").expect("match");
        assert!(!entry.admin);
    }

    #[test]
    fn check_is_case_insensitive() {
        let (_tmp, path) =
            write_allow_file("[[users]]\nemail = \"Alice@Example.COM\"\nadmin = false\n");

        let list = AccessList::load_from_file(&path, Environment::Development).unwrap();

        assert!(list.check("alice@example.com").is_some());
        assert!(list.check("ALICE@EXAMPLE.COM").is_some());
        assert!(list.check("Alice@Example.com").is_some());
    }

    #[test]
    fn check_trims_whitespace_on_lookup() {
        let (_tmp, path) = write_allow_file(
            "[[users]]\nemail = \"alice@example.com\"\nadmin = true\n",
        );

        let list = AccessList::load_from_file(&path, Environment::Development).unwrap();

        assert!(list.check("  alice@example.com  ").is_some());
    }

    #[test]
    fn empty_file_in_development_loads_as_empty_list() {
        let (_tmp, path) = write_allow_file("");

        let list = AccessList::load_from_file(&path, Environment::Development).unwrap();

        assert_eq!(list.len(), 0);
        assert!(list.check("anyone@example.com").is_none());
    }

    #[test]
    fn missing_users_table_in_development_loads_as_empty_list() {
        // Valid TOML with no `[[users]]` entries at all.
        let (_tmp, path) = write_allow_file("# no users here\n");

        let list = AccessList::load_from_file(&path, Environment::Development).unwrap();

        assert_eq!(list.len(), 0);
    }

    #[test]
    fn emails_are_normalized_on_ingest_even_with_wrapping_whitespace() {
        let (_tmp, path) = write_allow_file(
            "[[users]]\nemail = \"  Bob@Example.com  \"\nadmin = false\n",
        );

        let list = AccessList::load_from_file(&path, Environment::Development).unwrap();

        // Lookup succeeds with the trimmed lowercased form.
        assert!(list.check("bob@example.com").is_some());
    }

    #[test]
    fn empty_email_string_is_rejected() {
        let (_tmp, path) = write_allow_file("[[users]]\nemail = \"\"\nadmin = false\n");

        let err = AccessList::load_from_file(&path, Environment::Development).unwrap_err();

        assert!(
            format!("{err:#}").contains("empty email"),
            "unexpected error message: {err:#}"
        );
    }

    #[test]
    fn duplicate_emails_are_rejected_case_insensitively() {
        let (_tmp, path) = write_allow_file(
            "[[users]]\n\
             email = \"alice@example.com\"\n\
             admin = false\n\
             [[users]]\n\
             email = \"ALICE@EXAMPLE.COM\"\n\
             admin = true\n",
        );

        let err = AccessList::load_from_file(&path, Environment::Development).unwrap_err();

        assert!(
            format!("{err:#}").contains("duplicate"),
            "unexpected error message: {err:#}"
        );
    }

    #[test]
    fn empty_list_in_production_is_boot_error() {
        let (_tmp, path) = write_allow_file("");

        let err = AccessList::load_from_file(&path, Environment::Production).unwrap_err();

        assert!(
            format!("{err:#}").contains("nobody could sign in"),
            "unexpected error message: {err:#}"
        );
    }

    #[test]
    fn malformed_toml_is_rejected() {
        let (_tmp, path) = write_allow_file("this is not valid toml = = =");

        AccessList::load_from_file(&path, Environment::Development).unwrap_err();
    }

    #[test]
    fn check_of_absent_email_returns_none() {
        let (_tmp, path) = write_allow_file(
            "[[users]]\nemail = \"alice@example.com\"\nadmin = false\n",
        );

        let list = AccessList::load_from_file(&path, Environment::Development).unwrap();

        assert!(list.check("mallory@example.com").is_none());
    }

    #[test]
    fn missing_file_produces_context_error() {
        let err = AccessList::load_from_file(
            "/definitely/does/not/exist/authorized-users.toml",
            Environment::Development,
        )
        .unwrap_err();

        let msg = format!("{err:#}");
        assert!(
            msg.contains("reading"),
            "expected `reading <path>` context, got: {msg}"
        );
    }
}

