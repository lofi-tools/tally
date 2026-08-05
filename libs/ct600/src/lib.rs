//! CT600 corporation-tax return crate.
//!
//! Builds the CT600 return ([`Ct600Return`]) from computed iXBRL inputs, with
//! the GovTalk API client ([`govtalk`]) for the messages exchanged with HMRC
//! and the HTTP client ([`clients::hmrc_corp_tax`]) that runs the full
//! Document Submission Protocol lifecycle (submit, poll, delete) against the
//! External Test Service, the live service, or a Test-in-live submission.
//! The Companies House client, its layered [`Config`], the company
//! resolution chain and the CT600-specific adapters all live in
//! [`companies_house`].

pub mod clients;
pub use clients::{
    HmrcCorpTaxClient, HmrcCorpTaxConfig, HmrcCorpTaxError, HmrcResult, SubmissionOutcome,
    SubmissionReceipt,
};
pub mod companies_house;
pub use companies_house::{
    ApiResult, CompaniesHouseClient, CompaniesHouseClientType, CompaniesHouseError, CompanyProfile,
    CompanyType, Config, next_accounting_period_from,
};
pub mod ct600_return;
pub use ct600_return::Ct600Return;
pub mod form;
pub use form::{BoxValue, CompanyFormValues, Ct600FormValues, FieldValue};
#[cfg(test)]
pub mod test_utils;

pub mod govtalk;
pub use govtalk::{
    // namespaces
    ENV_NS, CT_NS, SR_NS,
    // envelope structures
    GovTalkEnvelope, Header, MessageDetails, ResponseEndpoint, SenderDetails,
    IDAuthentication, Authentication, GovTalkDetails, Keys, Key, TargetDetails,
    ChannelRouting, Channel, GovTalkErrors, Error, Body, IRenvelope, IRHeader,
    IRMark, SuccessResponse,
    // parameters + message trait
    GovTalkParams, Message,
    // message types
    GovTalkSubmissionRequest, GovTalkSubmissionAcknowledgement, GovTalkSubmissionPoll,
    GovTalkSubmissionError, GovTalkSubmissionResponse, GovTalkDeleteRequest,
    GovTalkDeleteResponse,
    // decoder
    decode_govtalk_message, GovTalkMessage,
    // errors
    Ct600Error, Result,
};
