#![cfg(test)]
//! Test utilities for the ct600 crate.
//!
//! The company fixtures and offline test clients live in
//! `ixbrl::clients::test_utils`; [`sample_values`] derives the CT600 form
//! values from the shared sample tax computation.

use crate::form::Ct600FormValues;

/// The CT600 form values derived from the shared sample tax computation
/// (`ixbrl::clients::test_utils::TestData::sample_tax`).
pub fn sample_values() -> Ct600FormValues {
    Ct600FormValues::from_tax(&ixbrl::clients::test_utils::TestData::sample_tax())
}
