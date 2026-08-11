//! `tally-api`: the Tally web API (spec: `docs/spec/api-backend-spec.md`).
//!
//! The library target exists so the pg-gated integration tests (`tests/`)
//! can build a real app against a Postgres; the binary (`main.rs`) is a thin
//! env/tracing/bind wrapper around [`app::router`].

pub mod app;
pub mod auth;
pub mod companies;
pub mod companies_house;
pub mod error;
pub mod extract;
pub mod ledgers;
pub mod models;
pub mod period;
pub mod reports;
