//! HMRC Corporation Tax online service.
//!
//! CT600 returns are filed with HMRC through the *Transaction Engine* using
//! the Document Submission Protocol (DSP): a GovTalk XML message is POSTed to
//! a submission endpoint, an acknowledgement carries a correlation ID and a
//! poll interval, the client polls a response endpoint until the final
//! response (success or error) arrives, and a delete request closes the
//! conversation.
//!
//! [`govtalk`] provides the GovTalk message envelope model (the XML messages,
//! the IRmark, and the decoder); [`dsp`] provides the HTTP client that runs
//! the DSP lifecycle, generic over the submission type via
//! [`SubmissionMessage`] (the ct600 crate implements it for its return).
//! The embedded company-resolution config comes from the
//! [`companies-house`] client crate ([`IxbrlConfig`]).

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

pub mod dsp;
pub use dsp::{
    CLASS_LIVE, CLASS_TEST_IN_LIVE, ETS_BASE_URL, HmrcCorpTaxClient, HmrcCorpTaxConfig,
    HmrcCorpTaxError, HmrcResult, IxbrlConfig, SubmissionEnvelope, SubmissionMessage,
    SubmissionOutcome, SubmissionReceipt, compute_irmark, inject_irmark,
};
