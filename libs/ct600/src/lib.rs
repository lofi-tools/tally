//! CT600 corporation-tax return crate.
//!
//! Builds the CT600 return ([`Ct600Return`]) from computed iXBRL inputs, with
//! the GovTalk messages and the Document Submission Protocol client provided
//! by the [`hmrc_corp_tax`] client crate (submit, poll, delete against the
//! External Test Service, the live service, or a Test-in-live submission;
//! the return implements its [`SubmissionMessage`] trait).  The Companies
//! House client and its layered [`Config`] come from the `companies-house`
//! client crate (re-exported in [`companies_house`], with the CT600-specific
//! enrichment adapters).

pub use hmrc_corp_tax::{
    ENV_NS, CT_NS, SR_NS,
    GovTalkEnvelope, Header, MessageDetails, ResponseEndpoint, SenderDetails,
    IDAuthentication, Authentication, GovTalkDetails, Keys, Key, TargetDetails,
    ChannelRouting, Channel, GovTalkErrors, Error, Body, IRenvelope, IRHeader,
    IRMark, SuccessResponse,
    GovTalkParams, Message,
    GovTalkSubmissionRequest, GovTalkSubmissionAcknowledgement, GovTalkSubmissionPoll,
    GovTalkSubmissionError, GovTalkSubmissionResponse, GovTalkDeleteRequest,
    GovTalkDeleteResponse,
    decode_govtalk_message, GovTalkMessage,
    Ct600Error, Result,
    HmrcCorpTaxClient, HmrcCorpTaxConfig, HmrcCorpTaxError, HmrcResult, SubmissionEnvelope,
    SubmissionMessage, SubmissionOutcome, SubmissionReceipt,
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
