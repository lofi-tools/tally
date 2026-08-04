//! HMRC Corporation Tax online service client.
//!
//! CT600 returns are filed with HMRC through the *Transaction Engine* using
//! the Document Submission Protocol (DSP): a GovTalk XML message is POSTed to
//! a submission endpoint, an acknowledgement carries a correlation ID and a
//! poll interval, the client polls a response endpoint until the final
//! response (success or error) arrives, and a delete request closes the
//! conversation.
//!
//! Endpoints (from the official "How to use the test service" guidance and
//! the Transaction Engine DSP):
//!
//! * External Test Service (ETS): `https://test-transaction-engine.tax.service.gov.uk`
//! * Live: `https://transaction-engine.tax.service.gov.uk`
//!
//! A *Test-in-live* submission is a live submission that runs full validation
//! without registering the return: it uses the live endpoints and the message
//! class `HMRC-CT-CT600-TIL` instead of `HMRC-CT-CT600`.
//!
//! Credentials (username / password / vendor ID) are issued by the Software
//! Developers Support Team (SDST); the vendor ID is the `ChannelRouting`
//! `URI`.

use std::time::Duration;

use base64::{Engine as _, engine::general_purpose};
use chrono::Local;
use ixbrl::clients::Config as IxbrlConfig;
use reqwest::StatusCode;
use sha1::{Digest, Sha1};
use snafu::Snafu;

use crate::ct600_return::{Ct600Return, EnvelopeConfig};
use crate::govtalk::{
    GovTalkDeleteRequest, GovTalkMessage, GovTalkParams, GovTalkSubmissionPoll, Message,
    decode_govtalk_message,
};
use crate::{CT_NS, ENV_NS};
use ixbrl::ixbrl_fmt::XmlNode;

// ============================================================================
// Endpoints
// ============================================================================

/// Base URL of the External Test Service (ETS) Transaction Engine.
pub const ETS_BASE_URL: &str = "https://test-transaction-engine.tax.service.gov.uk";
/// Base URL of the live Transaction Engine.
pub const LIVE_BASE_URL: &str = "https://transaction-engine.tax.service.gov.uk";

/// The message class for live (and ETS) CT600 submissions.
pub const CLASS_LIVE: &str = "HMRC-CT-CT600";
/// The message class for Test-in-live submissions.
pub const CLASS_TEST_IN_LIVE: &str = "HMRC-CT-CT600-TIL";

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the HMRC Corporation Tax client.
///
/// Resolved from the environment at construction, layered like the embedded
/// [`IxbrlConfig`]: explicit `with_*` overrides win over the environment,
/// which wins over the built-in defaults.
///
/// Environment variables consulted:
///
/// * `HMRC_CT_SUBMISSION_URL` / `HMRC_CT_POLL_URL` — the submission and poll
///   endpoints (default to the ETS endpoints for [`Self::test_from_env`] and
///   the live endpoints for [`Self::live_from_env`] / [`Self::test_in_live_from_env`]);
/// * `HMRC_CT_CLASS` — the message class (`HMRC-CT-CT600`, or
///   `HMRC-CT-CT600-TIL` for Test-in-live);
/// * `HMRC_CT_USERNAME` / `HMRC_CT_PASSWORD` — the gateway credentials issued
///   by SDST;
/// * `HMRC_CT_VENDOR_ID` — the 4-digit vendor ID (the `ChannelRouting` URI);
/// * `HMRC_CT_SOFTWARE` / `HMRC_CT_SOFTWARE_VERSION` — the software product
///   name and version;
/// * `HMRC_CT_GATEWAY_TEST` — `1`/`true` for the test services, `0`/`false`
///   for live;
/// * `HMRC_CT_POLL_TIMEOUT` — the total polling timeout in seconds (default
///   120, matching the reference tool);
/// * `HMRC_CT_POLL_INTERVAL` — the default poll interval in seconds (default
///   1, only used when the acknowledgement carries no `PollInterval`);
/// * the [`IxbrlConfig`] variables (`COMPANY_NUMBER`,
///   `COMPANIES_HOUSE_API_KEY`, `CT600_CACHE_DIR`, ...) for the embedded
///   company-resolution config.
///
/// The `Debug` impl redacts `password`.
#[derive(Clone)]
pub struct HmrcCorpTaxConfig {
    /// The embedded ixbrl config (company resolution, Companies House API,
    /// cache directory) — the same layered config the reporting pipeline
    /// uses, since ct600 builds on ixbrl.
    pub ixbrl: IxbrlConfig,
    /// The submission endpoint (the full URL the return is POSTed to).
    pub submission_url: String,
    /// The poll endpoint (the full URL poll / delete messages go to).
    pub poll_url: String,
    /// The message class: `HMRC-CT-CT600` or `HMRC-CT-CT600-TIL`.
    pub class: String,
    /// The `GatewayTest` flag: `true` for the test services / Test-in-live.
    pub gateway_test: bool,
    /// The gateway SenderID.
    pub username: String,
    /// The gateway authentication value.
    pub password: String,
    /// The vendor ID (`ChannelRouting` `URI`).
    pub vendor_id: String,
    /// The software product name.
    pub software: String,
    /// The software product version.
    pub software_version: String,
    /// Total polling timeout.
    pub poll_timeout: Duration,
    /// Default poll interval (used when the ack carries no `PollInterval`).
    pub poll_interval: Duration,
}

/// The endpoint pair (submission + poll) for a base URL.
fn endpoints(base: &str) -> (String, String) {
    (format!("{base}/submission"), format!("{base}/poll"))
}

/// Redacts the gateway password in debug output.
impl std::fmt::Debug for HmrcCorpTaxConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HmrcCorpTaxConfig")
            .field("ixbrl", &self.ixbrl)
            .field("submission_url", &self.submission_url)
            .field("poll_url", &self.poll_url)
            .field("class", &self.class)
            .field("gateway_test", &self.gateway_test)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("vendor_id", &self.vendor_id)
            .field("software", &self.software)
            .field("software_version", &self.software_version)
            .field("poll_timeout", &self.poll_timeout)
            .field("poll_interval", &self.poll_interval)
            .finish()
    }
}

impl HmrcCorpTaxConfig {
    /// The ETS configuration: test endpoints, `GatewayTest=1` and the live
    /// class, with credentials from the environment (optional).
    pub fn test_from_env() -> Self {
        let (submission_url, poll_url) = endpoints(ETS_BASE_URL);
        Self {
            ixbrl: IxbrlConfig::from_env(),
            submission_url: env_or("HMRC_CT_SUBMISSION_URL", submission_url),
            poll_url: env_or("HMRC_CT_POLL_URL", poll_url),
            class: env_or("HMRC_CT_CLASS", CLASS_LIVE.to_string()),
            gateway_test: true,
            username: env_or("HMRC_CT_USERNAME", String::new()),
            password: env_or("HMRC_CT_PASSWORD", String::new()),
            vendor_id: env_or("HMRC_CT_VENDOR_ID", String::new()),
            software: env_or("HMRC_CT_SOFTWARE", "ct600".to_string()),
            software_version: env_or("HMRC_CT_SOFTWARE_VERSION", "1.0.0".to_string()),
            poll_timeout: Duration::from_secs(env_secs("HMRC_CT_POLL_TIMEOUT", 120)),
            poll_interval: Duration::from_secs(env_secs("HMRC_CT_POLL_INTERVAL", 1)),
        }
    }

    /// The live configuration: live endpoints, `GatewayTest=0` and the live
    /// class, with credentials from the environment (optional).
    pub fn live_from_env() -> Self {
        let mut config = Self::test_from_env();
        let (submission_url, poll_url) = endpoints(LIVE_BASE_URL);
        config.submission_url = env_or("HMRC_CT_SUBMISSION_URL", submission_url);
        config.poll_url = env_or("HMRC_CT_POLL_URL", poll_url);
        config.class = env_or("HMRC_CT_CLASS", CLASS_LIVE.to_string());
        config.gateway_test = env_gateway_test(false);
        config
    }

    /// The Test-in-live configuration: live endpoints and the
    /// `HMRC-CT-CT600-TIL` class (full validation, no registration).
    pub fn test_in_live_from_env() -> Self {
        let mut config = Self::live_from_env();
        config.class = env_or("HMRC_CT_CLASS", CLASS_TEST_IN_LIVE.to_string());
        config.gateway_test = env_gateway_test(true);
        config
    }

    /// Resolve from the environment with the live endpoints and the live
    /// class (the default for real filings).
    pub fn from_env() -> Self {
        Self::live_from_env()
    }

    // -- overrides ----------------------------------------------------------

    /// Override the submission endpoint.
    pub fn with_submission_url(mut self, url: impl Into<String>) -> Self {
        self.submission_url = url.into();
        self
    }

    /// Override the poll endpoint.
    pub fn with_poll_url(mut self, url: impl Into<String>) -> Self {
        self.poll_url = url.into();
        self
    }

    /// Override the message class (e.g. `HMRC-CT-CT600-TIL`).
    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.class = class.into();
        self
    }

    /// Override the `GatewayTest` flag.
    pub fn with_gateway_test(mut self, gateway_test: bool) -> Self {
        self.gateway_test = gateway_test;
        self
    }

    /// Override the gateway username.
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = username.into();
        self
    }

    /// Override the gateway password.
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = password.into();
        self
    }

    /// Override the vendor ID.
    pub fn with_vendor_id(mut self, vendor_id: impl Into<String>) -> Self {
        self.vendor_id = vendor_id.into();
        self
    }

    /// Override the software product name.
    pub fn with_software(mut self, software: impl Into<String>) -> Self {
        self.software = software.into();
        self
    }

    /// Override the software product version.
    pub fn with_software_version(mut self, version: impl Into<String>) -> Self {
        self.software_version = version.into();
        self
    }

    /// Override the total polling timeout.
    pub fn with_poll_timeout(mut self, timeout: Duration) -> Self {
        self.poll_timeout = timeout;
        self
    }

    /// Override the default poll interval.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Override the embedded ixbrl config.
    pub fn with_ixbrl(mut self, ixbrl: IxbrlConfig) -> Self {
        self.ixbrl = ixbrl;
        self
    }

    /// Point the client at unreachable endpoints, for tests that must never
    /// reach the network.
    #[cfg(test)]
    pub(crate) fn with_unreachable_endpoints(mut self) -> Self {
        self.submission_url = "http://127.0.0.1:1/submission".to_string();
        self.poll_url = "http://127.0.0.1:1/poll".to_string();
        self
    }

    // -- accessors ----------------------------------------------------------

    /// The embedded ixbrl config.
    pub fn ixbrl(&self) -> &IxbrlConfig {
        &self.ixbrl
    }
}

/// A non-empty environment variable, or a default.
fn env_or(name: &str, default: String) -> String {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or(default)
}

/// An environment variable interpreted as seconds, or a default.
fn env_secs(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The `GatewayTest` flag from the environment, or a default.
fn env_gateway_test(default: bool) -> bool {
    match std::env::var("HMRC_CT_GATEWAY_TEST") {
        Ok(v) => matches!(v.as_str(), "1" | "true" | "yes"),
        Err(_) => default,
    }
}

// ============================================================================
// Errors
// ============================================================================

/// Errors returned by the HMRC Corporation Tax client.
#[derive(Debug, Snafu)]
pub enum HmrcCorpTaxError {
    /// The HTTP request could not be sent.
    #[snafu(display("request to {url} failed: {source}"))]
    RequestFailed { url: String, source: reqwest::Error },

    /// The API returned a non-success status code.
    #[snafu(display("POST {url} returned HTTP {status}"))]
    HttpStatus { url: String, status: StatusCode },

    /// The response body could not be decoded as a GovTalk message.
    #[snafu(display("failed to decode GovTalk response from {url}: {source}"))]
    DecodeFailed {
        url: String,
        source: crate::govtalk::Ct600Error,
    },

    /// The submission message could not be built.
    #[snafu(display("failed to build submission message: {source}"))]
    BuildFailed { source: crate::govtalk::Ct600Error },

    /// The gateway returned a submission error.
    #[snafu(display("HMRC rejected the submission: error {number} ({kind}) {text}"))]
    SubmissionError {
        number: String,
        kind: String,
        text: String,
    },

    /// Polling timed out without a final response.
    #[snafu(display(
        "timed out after {timeout:?} waiting for a response (correlation id {correlation_id})"
    ))]
    PollTimeout {
        correlation_id: String,
        timeout: Duration,
    },

    /// The response endpoint was missing from the acknowledgement.
    #[snafu(display("the acknowledgement carried no response endpoint"))]
    MissingResponseEndpoint,
}

pub type HmrcResult<T> = std::result::Result<T, HmrcCorpTaxError>;

// ============================================================================
// Receipt + outcome
// ============================================================================

/// The acknowledgement returned for a submission: what to poll and how often.
#[derive(Debug, Clone)]
pub struct SubmissionReceipt {
    /// The correlation ID used for polling and deletion.
    pub correlation_id: String,
    /// The endpoint to send poll / delete messages to.
    pub response_endpoint: String,
    /// The poll interval (seconds), when the gateway specified one.
    pub poll_interval: Option<Duration>,
}

/// The final outcome of a submission.
#[derive(Debug, Clone)]
pub struct SubmissionOutcome {
    /// The correlation ID of the conversation.
    pub correlation_id: String,
    /// The success response message.
    pub message: String,
}

// ============================================================================
// Client
// ============================================================================

/// A client for the HMRC Corporation Tax online service.
///
/// The full Document Submission Protocol lifecycle is exposed as discrete
/// steps ([`Self::submit`] → [`Self::poll`] → [`Self::delete`]) and as a
/// single [`Self::submit_and_poll`] convenience method.
///
/// The submission message is built from a [`Ct600Return`] (its envelope
/// credentials are overridden with this client's config), the IRmark is
/// computed from the message body, and the message is POSTed to the
/// submission endpoint.
#[derive(Debug, Clone)]
pub struct HmrcCorpTaxClient {
    config: HmrcCorpTaxConfig,
    http: reqwest::Client,
}

impl HmrcCorpTaxClient {
    /// Build a client from a fully-resolved [`HmrcCorpTaxConfig`].
    pub fn new(config: HmrcCorpTaxConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }

    /// A client pointed at unreachable endpoints, for tests that must never
    /// reach the network.
    #[cfg(test)]
    pub(crate) fn offline() -> Self {
        Self::new(HmrcCorpTaxConfig::test_from_env().with_unreachable_endpoints())
    }

    /// The configuration this client was built with.
    pub fn config(&self) -> &HmrcCorpTaxConfig {
        &self.config
    }

    // -- lifecycle ----------------------------------------------------------

    /// Step 1: submit the return and receive the acknowledgement.
    ///
    /// The return's envelope is overridden with the client's credentials, the
    /// IRmark is computed and injected, and the message is POSTed to the
    /// submission endpoint.  Returns the receipt (correlation ID, response
    /// endpoint, poll interval) for polling.
    pub async fn submit(&self, ct600: &Ct600Return) -> HmrcResult<SubmissionReceipt> {
        let xml = self.build_submission_xml(ct600)?;
        let body = self.post(&self.config.submission_url, &xml).await?;
        self.receipt_from_response(&body)
    }

    /// Step 2: poll the response endpoint until a final response arrives.
    ///
    /// Sleeps the acknowledgement's poll interval between requests, up to the
    /// configured timeout.  Returns the final success outcome, or an error.
    pub async fn poll(&self, receipt: &SubmissionReceipt) -> HmrcResult<SubmissionOutcome> {
        let timeout = self.config.poll_timeout;
        let deadline = std::time::Instant::now() + timeout;
        let mut interval = receipt.poll_interval.unwrap_or(self.config.poll_interval);

        loop {
            tokio::time::sleep(interval).await;
            if std::time::Instant::now() > deadline {
                return Err(HmrcCorpTaxError::PollTimeout {
                    correlation_id: receipt.correlation_id.clone(),
                    timeout,
                });
            }
            let poll = GovTalkSubmissionPoll::new(self.poll_params(&receipt.correlation_id));
            let xml = poll
                .to_xml()
                .map_err(|source| HmrcCorpTaxError::BuildFailed { source })?;
            let body = self.post(&receipt.response_endpoint, &xml).await?;
            match decode_govtalk_message(&body).map_err(|source| {
                HmrcCorpTaxError::DecodeFailed {
                    url: receipt.response_endpoint.clone(),
                    source,
                }
            })? {
                GovTalkMessage::SubmissionAcknowledgement(ack) => {
                    if let Ok(secs) = ack.params.poll_interval.parse::<f64>()
                        && secs > 0.0
                    {
                        interval = Duration::from_secs_f64(secs);
                    }
                }
                GovTalkMessage::SubmissionResponse(resp) => {
                    return Ok(SubmissionOutcome {
                        correlation_id: receipt.correlation_id.clone(),
                        message: resp.params.success_response_message.clone(),
                    });
                }
                GovTalkMessage::SubmissionError(err) => {
                    return Err(HmrcCorpTaxError::SubmissionError {
                        number: err.params.error_number.clone(),
                        kind: err.params.error_type.clone(),
                        text: err.params.error_text.clone(),
                    });
                }
                other => {
                    return Err(HmrcCorpTaxError::DecodeFailed {
                        url: receipt.response_endpoint.clone(),
                        source: crate::govtalk::Ct600Error::XmlError {
                            message: format!("unexpected poll response: {}", message_kind(&other)),
                        },
                    });
                }
            }
        }
    }

    /// Step 3: delete the submission from the gateway, closing the
    /// conversation.
    pub async fn delete(&self, receipt: &SubmissionReceipt) -> HmrcResult<()> {
        let delete = GovTalkDeleteRequest::new(self.delete_params(&receipt.correlation_id));
        let xml = delete
            .to_xml()
            .map_err(|source| HmrcCorpTaxError::BuildFailed { source })?;
        let body = self.post(&receipt.response_endpoint, &xml).await?;
        match decode_govtalk_message(&body).map_err(|source| HmrcCorpTaxError::DecodeFailed {
            url: receipt.response_endpoint.clone(),
            source,
        })? {
            GovTalkMessage::DeleteResponse(_) => Ok(()),
            GovTalkMessage::SubmissionError(err) => Err(HmrcCorpTaxError::SubmissionError {
                number: err.params.error_number.clone(),
                kind: err.params.error_type.clone(),
                text: err.params.error_text.clone(),
            }),
            other => Err(HmrcCorpTaxError::DecodeFailed {
                url: receipt.response_endpoint.clone(),
                source: crate::govtalk::Ct600Error::XmlError {
                    message: format!("unexpected delete response: {}", message_kind(&other)),
                },
            }),
        }
    }

    /// The full lifecycle: submit, poll to completion, then delete.
    ///
    /// The conversation is closed with a best-effort delete whether polling
    /// succeeds or fails (matching the reference tool).
    pub async fn submit_and_poll(&self, ct600: &Ct600Return) -> HmrcResult<SubmissionOutcome> {
        let receipt = self.submit(ct600).await?;
        let outcome = self.poll(&receipt).await;
        let _ = self.delete(&receipt).await;
        outcome
    }

    // -- message building ---------------------------------------------------

    /// Build the submission XML: the [`Ct600Return`] message with the
    /// client's envelope credentials and a computed IRmark.
    pub fn build_submission_xml(&self, ct600: &Ct600Return) -> HmrcResult<String> {
        let config = &self.config;
        let mut ct600 = ct600.clone();
        ct600.envelope = EnvelopeConfig {
            class: config.class.clone(),
            qualifier: "request".to_string(),
            function: "submit".to_string(),
            gateway_test: if config.gateway_test { "1" } else { "0" }.to_string(),
            username: config.username.clone(),
            password: config.password.clone(),
            vendor_id: config.vendor_id.clone(),
            software: config.software.clone(),
            software_version: config.software_version.clone(),
            timestamp: Local::now().naive_local(),
        };
        let mut xml = ct600.to_xml();
        let irmark = compute_irmark(&xml).map_err(|e| HmrcCorpTaxError::BuildFailed {
            source: crate::govtalk::Ct600Error::XmlError { message: e },
        })?;
        inject_irmark(&mut xml, &irmark);
        Ok(xml)
    }

    /// The poll / delete message parameters for a correlation ID.
    fn poll_params(&self, correlation_id: &str) -> GovTalkParams {
        self.conversation_params("submit", "poll", correlation_id)
    }

    /// The delete message parameters for a correlation ID.
    fn delete_params(&self, correlation_id: &str) -> GovTalkParams {
        self.conversation_params("delete", "request", correlation_id)
    }

    /// Message parameters for poll / delete conversations.
    fn conversation_params(
        &self,
        function: &str,
        qualifier: &str,
        correlation_id: &str,
    ) -> GovTalkParams {
        GovTalkParams {
            class: self.config.class.clone(),
            function: function.to_string(),
            qualifier: qualifier.to_string(),
            username: self.config.username.clone(),
            password: self.config.password.clone(),
            vendor_id: self.config.vendor_id.clone(),
            software: self.config.software.clone(),
            software_version: self.config.software_version.clone(),
            gateway_test: if self.config.gateway_test { "1" } else { "0" }.to_string(),
            correlation_id: correlation_id.to_string(),
            timestamp: Some(Local::now()),
            ..Default::default()
        }
    }

    /// POST an XML body and return the response text.
    async fn post(&self, url: &str, xml: &str) -> HmrcResult<String> {
        let response = self
            .http
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/xml")
            .header(reqwest::header::ACCEPT, "application/xml")
            .body(xml.to_string())
            .send()
            .await
            .map_err(|source| HmrcCorpTaxError::RequestFailed {
                url: url.to_string(),
                source,
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(HmrcCorpTaxError::HttpStatus {
                url: url.to_string(),
                status,
            });
        }
        response
            .text()
            .await
            .map_err(|source| HmrcCorpTaxError::RequestFailed {
                url: url.to_string(),
                source,
            })
    }

    /// Decode a response into a submission receipt.
    fn receipt_from_response(&self, body: &str) -> HmrcResult<SubmissionReceipt> {
        match decode_govtalk_message(body).map_err(|source| HmrcCorpTaxError::DecodeFailed {
            url: self.config.submission_url.clone(),
            source,
        })? {
            GovTalkMessage::SubmissionAcknowledgement(ack) => {
                let response_endpoint = if ack.params.response_endpoint.is_empty() {
                    self.config.poll_url.clone()
                } else {
                    ack.params.response_endpoint.clone()
                };
                if response_endpoint.is_empty() {
                    return Err(HmrcCorpTaxError::MissingResponseEndpoint);
                }
                let poll_interval = ack
                    .params
                    .poll_interval
                    .parse::<f64>()
                    .ok()
                    .filter(|s| *s > 0.0)
                    .map(Duration::from_secs_f64);
                Ok(SubmissionReceipt {
                    correlation_id: ack.params.correlation_id.clone(),
                    response_endpoint,
                    poll_interval,
                })
            }
            GovTalkMessage::SubmissionError(err) => Err(HmrcCorpTaxError::SubmissionError {
                number: err.params.error_number.clone(),
                kind: err.params.error_type.clone(),
                text: err.params.error_text.clone(),
            }),
            other => Err(HmrcCorpTaxError::DecodeFailed {
                url: self.config.submission_url.clone(),
                source: crate::govtalk::Ct600Error::XmlError {
                    message: format!("unexpected submit response: {}", message_kind(&other)),
                },
            }),
        }
    }
}

// ============================================================================
// IRmark
// ============================================================================

/// Compute the IRmark for a CT600 GovTalk message: the SHA-1 of the
/// canonicalised `<Body>` content (with the IRmark elements removed),
/// base64-encoded, as described in the "HMRC (IR) mark: support for software
/// developers" technical pack and implemented by the reference `ct600` tool.
///
/// The reference tool rewraps the `<Body>` children (the `ct:IRenvelope`) in a
/// fresh `<Body>` root carrying the `xmlns` / `xmlns:ct` declarations, then
/// canonicalises that — so the body is hashed with its namespace context
/// intact.
fn compute_irmark(xml: &str) -> Result<String, String> {
    let node = XmlNode::from_xml_string(xml)?;
    let body = find_child(&node, "Body").ok_or("no <Body> element")?;
    let mut body = body.clone();
    strip_irmarks(&mut body);

    // The IRmark covers the canonical form of the body re-wrapped in a fresh
    // `<Body>` root with the envelope namespaces declared, mirroring the
    // reference `ct600` tool exactly.
    let mut wrapped = elt("Body", &[("xmlns", ENV_NS), ("xmlns:ct", CT_NS)]);
    wrapped.push_children(match body {
        XmlNode::Elem { children, .. } => children,
        _ => Vec::new(),
    });
    let canonical = canonicalize_node(&wrapped)?;
    let digest = Sha1::digest(canonical);
    Ok(general_purpose::STANDARD.encode(digest))
}

/// Remove the `ct:IRmark` elements that are direct children of a
/// `ct:IRheader`, matching the reference tool's removal logic.
fn strip_irmarks(node: &mut XmlNode) {
    if let XmlNode::Elem { name, children, .. } = node {
        if name == "ct:IRheader" {
            children.retain(|c| !matches!(c, XmlNode::Elem { name: n, .. } if n == "ct:IRmark"));
        }
        for child in children {
            strip_irmarks(child);
        }
    }
}

/// The first direct child element with the given name.
fn find_child<'a>(node: &'a XmlNode, name: &str) -> Option<&'a XmlNode> {
    match node {
        XmlNode::Elem { children, .. } => children
            .iter()
            .find(|c| matches!(c, XmlNode::Elem { name: n, .. } if n == name)),
        _ => None,
    }
}

/// Create an element with no children (namespace-context helper).
fn elt(name: &str, attrs: &[(&str, &str)]) -> XmlNode {
    XmlNode::Elem {
        name: name.to_string(),
        attributes: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        children: Vec::new(),
    }
}

/// The canonical (C14N) form of a node tree.
fn canonicalize_node(node: &XmlNode) -> Result<Vec<u8>, String> {
    let xml = node.to_xml_string();
    canonicalize_str(&xml)
}

/// The canonical (C14N) form of an XML string.
fn canonicalize_str(xml: &str) -> Result<Vec<u8>, String> {
    use bergshamra_c14n::{C14nMode, canonicalize};
    canonicalize(xml, C14nMode::Inclusive, None, &[] as &[String]).map_err(|e| e.to_string())
}

/// A short human-readable kind for an unexpected message type.
fn message_kind(message: &GovTalkMessage) -> &'static str {
    match message {
        GovTalkMessage::SubmissionRequest(_) => "submission request",
        GovTalkMessage::SubmissionAcknowledgement(_) => "acknowledgement",
        GovTalkMessage::SubmissionPoll(_) => "poll",
        GovTalkMessage::SubmissionError(_) => "error",
        GovTalkMessage::SubmissionResponse(_) => "response",
        GovTalkMessage::DeleteRequest(_) => "delete request",
        GovTalkMessage::DeleteResponse(_) => "delete response",
    }
}

/// Inject a computed IRmark into every `ct:IRmark` element of a message.
fn inject_irmark(xml: &mut String, irmark: &str) {
    if let Ok(mut node) = XmlNode::from_xml_string(xml) {
        set_irmarks(&mut node, irmark);
        if let XmlNode::Elem { .. } = node {
            *xml = format!(
                "<?xml version='1.0' encoding='UTF-8'?>\n{}",
                node.to_xml_string()
            );
        }
    }
}

/// Set the text of every `ct:IRmark` element in a tree, ensuring the
/// `Type="generic"` attribute the reference tool writes on each injected
/// mark is present.
fn set_irmarks(node: &mut XmlNode, irmark: &str) {
    if let XmlNode::Elem {
        name,
        attributes,
        children,
        ..
    } = node
    {
        if name == "ct:IRmark" {
            *children = vec![XmlNode::Text(irmark.to_string())];
            if !attributes.iter().any(|(k, _)| k == "Type") {
                attributes.push(("Type".to_string(), "generic".to_string()));
            }
        }
        for child in children {
            set_irmarks(child, irmark);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::govtalk::{GovTalkSubmissionAcknowledgement, GovTalkSubmissionError};
    use crate::test_utils::{StubGateway, TestData};

    /// The built message carries the configured credentials, the test gateway
    /// flag, and a non-empty IRmark injected into every `ct:IRmark` element.
    #[test]
    fn build_submission_xml_overrides_envelope_and_injects_irmark() {
        let client = HmrcCorpTaxClient::new(TestData::test_config().with_unreachable_endpoints());
        let xml = client
            .build_submission_xml(&TestData::sample_return())
            .expect("build");

        assert!(xml.contains("<Class>HMRC-CT-CT600</Class>"));
        assert!(xml.contains("<GatewayTest>1</GatewayTest>"));
        assert!(xml.contains("<SenderID>testuser</SenderID>"));
        assert!(xml.contains("<Value>testpass</Value>"));
        assert!(xml.contains("<URI>1234</URI>"));

        // The IRmark is computed and injected: the element is no longer empty.
        let node = XmlNode::from_xml_string(&xml).expect("parse");
        let mut found = false;
        collect_irmark_values(&node, &mut found);
        assert!(found, "expected a non-empty ct:IRmark value");
    }

    fn collect_irmark_values(node: &XmlNode, found: &mut bool) {
        if let XmlNode::Elem { name, children, .. } = node {
            if name == "ct:IRmark"
                && let Some(XmlNode::Text(t)) = children.first()
                && !t.is_empty()
            {
                *found = true;
            }
            for child in children {
                collect_irmark_values(child, found);
            }
        }
    }

    /// The IRmark is deterministic for a fixed message and changes when the
    /// message body changes.
    #[test]
    fn irmark_is_deterministic_and_body_sensitive() {
        let client = HmrcCorpTaxClient::new(TestData::test_config().with_unreachable_endpoints());
        let xml1 = client
            .build_submission_xml(&TestData::sample_return())
            .expect("build");
        let xml2 = client
            .build_submission_xml(&TestData::sample_return())
            .expect("build");
        assert_eq!(irmark_of(&xml1), irmark_of(&xml2));

        let mut other = TestData::sample_return();
        other.turnover += 1.0;
        let xml3 = client.build_submission_xml(&other).expect("build");
        assert_ne!(irmark_of(&xml1), irmark_of(&xml3));
    }

    /// The IRmark for the sample return matches the value produced by the
    /// reference `ct600` Python tool's `get_irmark` / `irmark.compute`
    /// (verified by running that algorithm on the built message with lxml).
    /// The envelope timestamp is in the header, so the body hash is stable.
    #[test]
    fn irmark_matches_reference_tool() {
        let client = HmrcCorpTaxClient::new(TestData::test_config().with_unreachable_endpoints());
        let xml = client
            .build_submission_xml(&TestData::sample_return())
            .expect("build");
        assert_eq!(irmark_of(&xml), "FFRGJ9wKKopilSidXDMBBZ5AXAY=");
    }

    fn irmark_of(xml: &str) -> String {
        let node = XmlNode::from_xml_string(xml).expect("parse");
        let mut out = String::new();
        find_irmark(&node, &mut out);
        out
    }

    fn find_irmark(node: &XmlNode, out: &mut String) {
        if let XmlNode::Elem { name, children, .. } = node {
            if name == "ct:IRmark"
                && let Some(XmlNode::Text(t)) = children.first()
            {
                *out = t.clone();
            }
            for child in children {
                find_irmark(child, out);
            }
        }
    }

    /// A response with `Qualifier=acknowledgement` decodes into a receipt.
    #[test]
    fn receipt_from_acknowledgement() {
        let client = HmrcCorpTaxClient::offline();
        let ack = GovTalkSubmissionAcknowledgement::new(GovTalkParams {
            class: CLASS_LIVE.to_string(),
            function: "submit".to_string(),
            qualifier: "acknowledgement".to_string(),
            correlation_id: "CORR-1".to_string(),
            response_endpoint: "http://localhost:8080/poll".to_string(),
            poll_interval: "2".to_string(),
            ..Default::default()
        });
        let xml = ack.to_xml().expect("serialize ack");

        let receipt = client.receipt_from_response(&xml).expect("receipt");
        assert_eq!(receipt.correlation_id, "CORR-1");
        assert_eq!(receipt.response_endpoint, "http://localhost:8080/poll");
        assert_eq!(receipt.poll_interval, Some(Duration::from_secs(2)));
    }

    /// A `Qualifier=error` response surfaces as a typed error.
    #[test]
    fn receipt_surfaces_gateway_error() {
        let client = HmrcCorpTaxClient::offline();
        let err = GovTalkSubmissionError::new(GovTalkParams {
            class: CLASS_LIVE.to_string(),
            function: "submit".to_string(),
            qualifier: "error".to_string(),
            error_number: "1001".to_string(),
            error_type: "business".to_string(),
            error_text: "Box 145 is invalid".to_string(),
            ..Default::default()
        });
        let xml = err.to_xml().expect("serialize error");

        match client.receipt_from_response(&xml) {
            Err(HmrcCorpTaxError::SubmissionError { number, kind, text }) => {
                assert_eq!(number, "1001");
                assert_eq!(kind, "business");
                assert_eq!(text, "Box 145 is invalid");
            }
            other => panic!("expected SubmissionError, got {other:?}"),
        }
    }

    /// The config constructors resolve the expected endpoints, class and
    /// gateway-test flag.
    #[test]
    fn config_constructors_resolve_endpoints() {
        let test = HmrcCorpTaxConfig::test_from_env();
        assert_eq!(test.submission_url, format!("{ETS_BASE_URL}/submission"));
        assert_eq!(test.poll_url, format!("{ETS_BASE_URL}/poll"));
        assert_eq!(test.class, CLASS_LIVE);
        assert!(test.gateway_test);

        let live = HmrcCorpTaxConfig::live_from_env();
        assert_eq!(live.submission_url, format!("{LIVE_BASE_URL}/submission"));
        assert_eq!(live.poll_url, format!("{LIVE_BASE_URL}/poll"));
        assert_eq!(live.class, CLASS_LIVE);
        assert!(!live.gateway_test);

        let til = HmrcCorpTaxConfig::test_in_live_from_env();
        assert_eq!(til.submission_url, format!("{LIVE_BASE_URL}/submission"));
        assert_eq!(til.class, CLASS_TEST_IN_LIVE);
        assert!(til.gateway_test);
    }

    /// The config embeds a resolved ixbrl config.
    #[test]
    fn config_embeds_ixbrl_config() {
        let config = HmrcCorpTaxConfig::test_from_env();
        // The embedded config resolves the ixbrl defaults (no company number
        // set in this test environment).
        assert_eq!(config.ixbrl.company_number(), None);
    }

    /// The full lifecycle against an in-process stub gateway: submit, poll,
    /// respond, delete.
    #[tokio::test]
    async fn full_lifecycle_against_stub_gateway() {
        let stub = StubGateway::spawn().await;

        let client = HmrcCorpTaxClient::new(
            TestData::test_config()
                .with_submission_url(format!("{}/submission", stub.base))
                .with_poll_url(format!("{}/poll", stub.base)),
        );

        let outcome = client
            .submit_and_poll(&TestData::sample_return())
            .await
            .expect("lifecycle");
        assert_eq!(outcome.correlation_id, "CORR-1");
        assert_eq!(outcome.message, "Submission processed successfully");
        assert!(stub.deleted(), "the delete request should have been sent");
    }

    /// A gateway error during polling surfaces as a typed error.
    #[tokio::test]
    async fn polling_surfaces_gateway_error() {
        let stub = StubGateway::spawn().await;
        stub.reject_polls
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let client = HmrcCorpTaxClient::new(
            TestData::test_config()
                .with_submission_url(format!("{}/submission", stub.base))
                .with_poll_url(format!("{}/poll", stub.base))
                .with_poll_timeout(Duration::from_secs(10)),
        );

        let receipt = client.submit(&TestData::sample_return()).await.expect("submit");
        match client.poll(&receipt).await {
            Err(HmrcCorpTaxError::SubmissionError { text, .. }) => {
                assert_eq!(text, "Box 145 is invalid");
            }
            other => panic!("expected SubmissionError, got {other:?}"),
        }
    }
}
