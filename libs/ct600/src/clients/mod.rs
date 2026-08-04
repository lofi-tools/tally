//! HMRC filing clients.
//!
//! [`hmrc_corp_tax`] provides the HTTP client for the Corporation Tax online
//! service — the GovTalk submission lifecycle (submit, poll, delete) against
//! the External Test Service (ETS), the live service, or a Test-in-live
//! submission.

pub mod hmrc_corp_tax;
pub use hmrc_corp_tax::{
    HmrcCorpTaxClient, HmrcCorpTaxConfig, HmrcCorpTaxError, HmrcResult, SubmissionOutcome,
    SubmissionReceipt,
};
