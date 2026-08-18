#![cfg(test)]
//! Test utilities for the ixbrl crate.
//!
//! The repo-root resolution ([`REPO`], [`repo_path`]) and the hardcoded
//! fixtures ([`TestData`] — the fictional example company, its accounts and
//! the example GnuCash book path) live in the shared leaf `test_utils` crate;
//! this module re-exports them at their historical names so the crate's
//! tests keep working unchanged.
//!
//! Tests run with zero configuration on a fresh checkout.

pub use test_utils::{Fixtures as TestData, REPO, cache_dir, cache_path, repo_path};
