//! GovTalk message building and parsing module
//! Uses Serde for XML serialization/deserialization

use base64::{Engine as _, engine::general_purpose};
use bergshamra_c14n::{C14nMode, canonicalize};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

mod error;
pub use error::{Ct600Error, Result};
pub mod companies_house;
pub use companies_house::{CompaniesHouseClient, CompanyType};
pub mod form;
pub use form::{BoxValue, CompanyFormValues, Ct600FormValues, FieldValue};
#[cfg(test)]
pub mod test_utils;

// ============================================================================
// Namespace Definitions
// ============================================================================

pub const ENV_NS: &str = "http://www.govtalk.gov.uk/CM/envelope";
pub const CT_NS: &str = "http://www.govtalk.gov.uk/taxation/CT/5";
pub const SR_NS: &str = "http://www.inlandrevenue.gov.uk/SuccessResponse";

// ============================================================================
// Core GovTalk Envelope Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "GovTalkMessage")]
pub struct GovTalkEnvelope {
    #[serde(rename = "EnvelopeVersion")]
    pub envelope_version: String,
    #[serde(rename = "Header")]
    pub header: Header,
    #[serde(rename = "GovTalkDetails")]
    pub govtalk_details: GovTalkDetails,
    #[serde(rename = "Body")]
    pub body: Body,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    #[serde(rename = "MessageDetails")]
    pub message_details: MessageDetails,
    #[serde(rename = "SenderDetails")]
    pub sender_details: SenderDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDetails {
    #[serde(rename = "Class")]
    pub class: String,
    #[serde(rename = "Qualifier")]
    pub qualifier: String,
    #[serde(rename = "Function")]
    pub function: String,
    #[serde(
        rename = "TransactionID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub transaction_id: Option<String>,
    #[serde(
        rename = "CorrelationID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub correlation_id: Option<String>,
    #[serde(rename = "Transformation")]
    pub transformation: String,
    #[serde(rename = "GatewayTest")]
    pub gateway_test: String,
    #[serde(
        rename = "ResponseEndPoint",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub response_endpoint: Option<ResponseEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEndpoint {
    #[serde(
        rename = "PollInterval",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub poll_interval: Option<String>,
    #[serde(rename = "$value", default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderDetails {
    #[serde(rename = "IDAuthentication")]
    pub id_authentication: IDAuthentication,
    #[serde(
        rename = "EmailAddress",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub email_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDAuthentication {
    #[serde(rename = "SenderID")]
    pub sender_id: String,
    #[serde(rename = "Authentication")]
    pub authentication: Authentication,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authentication {
    #[serde(rename = "Method")]
    pub method: String,
    #[serde(rename = "Role")]
    pub role: String,
    #[serde(rename = "Value")]
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovTalkDetails {
    #[serde(rename = "Keys")]
    pub keys: Keys,
    #[serde(rename = "TargetDetails")]
    pub target_details: TargetDetails,
    #[serde(rename = "ChannelRouting")]
    pub channel_routing: ChannelRouting,
    #[serde(
        rename = "GovTalkErrors",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub govtalk_errors: Option<GovTalkErrors>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keys {
    #[serde(rename = "Key")]
    pub key: Key,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Key {
    #[serde(rename = "@Type")]
    pub r#type: String,
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetDetails {
    #[serde(rename = "Organisation")]
    pub organisation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRouting {
    #[serde(rename = "Channel")]
    pub channel: Channel,
    #[serde(rename = "Timestamp", default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    #[serde(rename = "URI")]
    pub uri: String,
    #[serde(rename = "Product")]
    pub product: String,
    #[serde(rename = "Version")]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovTalkErrors {
    #[serde(rename = "Error")]
    pub error: Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    #[serde(rename = "RaisedBy")]
    pub raised_by: String,
    #[serde(rename = "Number")]
    pub number: String,
    #[serde(rename = "Type")]
    pub r#type: String,
    #[serde(rename = "Text")]
    pub text: String,
    #[serde(rename = "Location", default)]
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    #[serde(
        rename = "IRenvelope",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub ir_envelope: Option<IRenvelope>,
    #[serde(
        rename = "SuccessResponse",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub success_response: Option<SuccessResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRenvelope {
    #[serde(rename = "IRheader")]
    pub ir_header: IRHeader,
    #[serde(rename = "IRmark", default, skip_serializing_if = "Option::is_none")]
    pub ir_mark: Option<IRMark>,
    #[serde(rename = "$value", default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRHeader {
    #[serde(rename = "IRmark")]
    pub ir_mark: IRMark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRMark {
    #[serde(rename = "@Type", default)]
    pub r#type: Option<String>,
    #[serde(rename = "$value")]
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessResponse {
    #[serde(rename = "Message")]
    pub message: String,
}

// ============================================================================
// Parameters Builder
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct GovTalkParams {
    pub class: String,
    pub function: String,
    pub qualifier: String,
    pub transaction_id: String,
    pub correlation_id: String,
    pub gateway_test: String,
    pub username: String,
    pub password: String,
    pub vendor_id: String,
    pub software: String,
    pub software_version: String,
    pub tax_reference: String,
    pub email: String,
    pub timestamp: Option<DateTime<Local>>,
    pub irmark: String,
    pub audit_id: String,
    pub ir_envelope_content: Option<String>,
    pub poll_interval: String,
    pub response_endpoint: String,
    pub error_number: String,
    pub error_type: String,
    pub error_text: String,
    pub error_location: String,
    pub success_response_message: String,
}

impl GovTalkParams {
    pub fn get(&self, key: &str) -> Option<&str> {
        match key {
            "class" => Some(&self.class),
            "function" => Some(&self.function),
            "qualifier" => Some(&self.qualifier),
            "transaction-id" => Some(&self.transaction_id),
            "correlation-id" => Some(&self.correlation_id),
            "gateway-test" => Some(&self.gateway_test),
            "username" => Some(&self.username),
            "password" => Some(&self.password),
            "vendor-id" => Some(&self.vendor_id),
            "software" => Some(&self.software),
            "software-version" => Some(&self.software_version),
            "tax-reference" => Some(&self.tax_reference),
            "email" => Some(&self.email),
            "irmark" => Some(&self.irmark),
            "audit-id" => Some(&self.audit_id),
            "poll-interval" => Some(&self.poll_interval),
            "response-endpoint" => Some(&self.response_endpoint),
            "error-number" => Some(&self.error_number),
            "error-type" => Some(&self.error_type),
            "error-text" => Some(&self.error_text),
            "error-location" => Some(&self.error_location),
            _ => None,
        }
    }
}

// ============================================================================
// Message Trait
// ============================================================================

pub trait Message {
    fn to_xml(&self) -> Result<String> {
        let envelope = self.create_message()?;
        let xml = quick_xml::se::to_string(&envelope)
            .map_err(|e| Ct600Error::XmlError {
                message: format!("Serialization error: {}", e),
            })?;
        Ok(format!(r#"<?xml version="1.0" encoding="UTF-8"?>{}"#, xml))
    }

    fn to_pretty_xml(&self) -> Result<String> {
        self.to_xml()
    }

    fn to_canonical_xml(&self) -> Result<String> {
        let xml = self.to_xml()?;
        Ok(xml.chars().filter(|c| !c.is_whitespace()).collect())
    }

    fn to_iso_date(date_str: &str) -> Result<String> {
        let date = NaiveDate::parse_from_str(date_str, "%d %B %Y")
            .map_err(|e| Ct600Error::ConfigError {
                message: format!("Invalid date format: {}", e),
            })?;
        Ok(date.format("%Y-%m-%d").to_string())
    }

    fn create_message(&self) -> Result<GovTalkEnvelope>;
}

// ============================================================================
// Message Type Implementations
// ============================================================================

// --- Helper: params <-> envelope conversion ---

fn params_from_envelope(envelope: &GovTalkEnvelope) -> Result<GovTalkParams> {
    let mut params = GovTalkParams {
        class: envelope.header.message_details.class.clone(),
        qualifier: envelope.header.message_details.qualifier.clone(),
        function: envelope.header.message_details.function.clone(),
        transaction_id: envelope
            .header
            .message_details
            .transaction_id
            .clone()
            .unwrap_or_default(),
        correlation_id: envelope
            .header
            .message_details
            .correlation_id
            .clone()
            .unwrap_or_default(),
        gateway_test: envelope.header.message_details.gateway_test.clone(),
        username: envelope
            .header
            .sender_details
            .id_authentication
            .sender_id
            .clone(),
        password: envelope
            .header
            .sender_details
            .id_authentication
            .authentication
            .value
            .clone(),
        email: envelope
            .header
            .sender_details
            .email_address
            .clone()
            .unwrap_or_default(),
        tax_reference: envelope.govtalk_details.keys.key.value.clone(),
        vendor_id: envelope.govtalk_details.channel_routing.channel.uri.clone(),
        software: envelope
            .govtalk_details
            .channel_routing
            .channel
            .product
            .clone(),
        software_version: envelope
            .govtalk_details
            .channel_routing
            .channel
            .version
            .clone(),
        ..Default::default()
    };

    if let Some(ref ts) = envelope.govtalk_details.channel_routing.timestamp
        && let Ok(dt) = DateTime::parse_from_rfc3339(ts)
    {
        params.timestamp = Some(dt.with_timezone(&Local));
    }

    if let Some(ref rep) = envelope.header.message_details.response_endpoint {
        params.poll_interval = rep.poll_interval.clone().unwrap_or_default();
        params.response_endpoint = rep.value.clone().unwrap_or_default();
    }

    if let Some(ref errors) = envelope.govtalk_details.govtalk_errors {
        params.error_number = errors.error.number.clone();
        params.error_type = errors.error.r#type.clone();
        params.error_text = errors.error.text.clone();
        params.error_location = errors.error.location.clone().unwrap_or_default();
    }

    if let Some(ref body) = envelope.body.ir_envelope
        && let Some(ref mark) = body.ir_mark
    {
        params.irmark = mark.value.clone();
    }

    if let Some(ref success) = envelope.body.success_response {
        params.success_response_message = success.message.clone();
    }

    Ok(params)
}

fn build_header(params: &GovTalkParams) -> Header {
    Header {
        message_details: MessageDetails {
            class: params.class.clone(),
            qualifier: params.qualifier.clone(),
            function: params.function.clone(),
            transaction_id: Some(params.transaction_id.clone()),
            correlation_id: Some(params.correlation_id.clone()),
            transformation: "XML".to_string(),
            gateway_test: params.gateway_test.clone(),
            response_endpoint: None,
        },
        sender_details: SenderDetails {
            id_authentication: IDAuthentication {
                sender_id: params.username.clone(),
                authentication: Authentication {
                    method: "clear".to_string(),
                    role: "principal".to_string(),
                    value: params.password.clone(),
                },
            },
            email_address: if params.email.is_empty() {
                None
            } else {
                Some(params.email.clone())
            },
        },
    }
}

fn build_govtalk_details(params: &GovTalkParams) -> GovTalkDetails {
    GovTalkDetails {
        keys: Keys {
            key: Key {
                r#type: "UTR".to_string(),
                value: params.tax_reference.clone(),
            },
        },
        target_details: TargetDetails {
            organisation: "HMRC".to_string(),
        },
        channel_routing: ChannelRouting {
            channel: Channel {
                uri: params.vendor_id.clone(),
                product: params.software.clone(),
                version: params.software_version.clone(),
            },
            timestamp: params.timestamp.as_ref().map(|ts| ts.to_rfc3339()),
        },
        govtalk_errors: None,
    }
}

// --- Submission Request ---

pub struct GovTalkSubmissionRequest {
    pub params: GovTalkParams,
}

impl GovTalkSubmissionRequest {
    pub fn new(params: GovTalkParams) -> Self {
        Self { params }
    }

    pub fn build_envelope(&self) -> Result<GovTalkEnvelope> {
        Ok(GovTalkEnvelope {
            envelope_version: "2.0".to_string(),
            header: build_header(&self.params),
            govtalk_details: build_govtalk_details(&self.params),
            body: Body {
                ir_envelope: Some(IRenvelope {
                    ir_header: IRHeader {
                        ir_mark: IRMark {
                            r#type: Some("generic".to_string()),
                            value: self.params.irmark.clone(),
                        },
                    },
                    ir_mark: Some(IRMark {
                        r#type: Some("generic".to_string()),
                        value: self.params.irmark.clone(),
                    }),
                    content: self.params.ir_envelope_content.clone(),
                }),
                success_response: None,
            },
        })
    }

    pub fn compute_irmark(&self) -> Result<String> {
        let content = self.params.ir_envelope_content.clone().unwrap_or_default();
        let irmark = &self.params.irmark;

        let mut w = uppsala::XmlWriter::new();
        w.start_element("IRenvelope", &[("xmlns", ENV_NS), ("xmlns:ct", CT_NS)]);
        w.start_element("ct:IRheader", &[]);
        w.start_element("ct:IRmark", &[("Type", "generic")]);
        w.text(irmark);
        w.end_element("ct:IRmark");
        w.end_element("ct:IRheader");
        w.start_element("ct:IRmark", &[("Type", "generic")]);
        w.text(irmark);
        w.end_element("ct:IRmark");
        w.raw(&content);
        w.end_element("IRenvelope");

        let xml = w.into_string();
        let canonical = canonicalize(&xml, C14nMode::Inclusive, None, &[] as &[String])
            .map_err(|e| Ct600Error::C14nError {
                message: e.to_string(),
            })?;

        let mut hasher = Sha1::new();
        hasher.update(&canonical);
        let result = hasher.finalize();
        Ok(general_purpose::STANDARD.encode(result))
    }
}

impl Serialize for GovTalkSubmissionRequest {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let envelope = self
            .build_envelope()
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        envelope.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GovTalkSubmissionRequest {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let envelope = GovTalkEnvelope::deserialize(deserializer)?;
        let params = params_from_envelope(&envelope).map_err(serde::de::Error::custom)?;
        Ok(Self { params })
    }
}

impl Message for GovTalkSubmissionRequest {
    fn create_message(&self) -> Result<GovTalkEnvelope> {
        self.build_envelope()
    }
}

// --- Submission Acknowledgement ---

pub struct GovTalkSubmissionAcknowledgement {
    pub params: GovTalkParams,
}

impl GovTalkSubmissionAcknowledgement {
    pub fn new(params: GovTalkParams) -> Self {
        Self { params }
    }
}

impl Serialize for GovTalkSubmissionAcknowledgement {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let envelope = self
            .create_message()
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        envelope.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GovTalkSubmissionAcknowledgement {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let envelope = GovTalkEnvelope::deserialize(deserializer)?;
        let params = params_from_envelope(&envelope).map_err(serde::de::Error::custom)?;
        Ok(Self { params })
    }
}

impl Message for GovTalkSubmissionAcknowledgement {
    fn create_message(&self) -> Result<GovTalkEnvelope> {
        let mut header = build_header(&self.params);
        header.message_details.response_endpoint = Some(ResponseEndpoint {
            poll_interval: Some(self.params.poll_interval.clone()),
            value: Some(self.params.response_endpoint.clone()),
        });
        Ok(GovTalkEnvelope {
            envelope_version: "2.0".to_string(),
            header,
            govtalk_details: build_govtalk_details(&self.params),
            body: Body {
                ir_envelope: None,
                success_response: None,
            },
        })
    }
}

// --- Submission Poll ---

pub struct GovTalkSubmissionPoll {
    pub params: GovTalkParams,
}

impl GovTalkSubmissionPoll {
    pub fn new(params: GovTalkParams) -> Self {
        Self { params }
    }
}

impl Serialize for GovTalkSubmissionPoll {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let envelope = self
            .create_message()
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        envelope.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GovTalkSubmissionPoll {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let envelope = GovTalkEnvelope::deserialize(deserializer)?;
        let params = params_from_envelope(&envelope).map_err(serde::de::Error::custom)?;
        Ok(Self { params })
    }
}

impl Message for GovTalkSubmissionPoll {
    fn create_message(&self) -> Result<GovTalkEnvelope> {
        Ok(GovTalkEnvelope {
            envelope_version: "2.0".to_string(),
            header: build_header(&self.params),
            govtalk_details: build_govtalk_details(&self.params),
            body: Body {
                ir_envelope: None,
                success_response: None,
            },
        })
    }
}

// --- Submission Error ---

pub struct GovTalkSubmissionError {
    pub params: GovTalkParams,
}

impl GovTalkSubmissionError {
    pub fn new(params: GovTalkParams) -> Self {
        Self { params }
    }
}

impl Serialize for GovTalkSubmissionError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let envelope = self
            .create_message()
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        envelope.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GovTalkSubmissionError {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let envelope = GovTalkEnvelope::deserialize(deserializer)?;
        let params = params_from_envelope(&envelope).map_err(serde::de::Error::custom)?;
        Ok(Self { params })
    }
}

impl Message for GovTalkSubmissionError {
    fn create_message(&self) -> Result<GovTalkEnvelope> {
        let mut details = build_govtalk_details(&self.params);
        details.govtalk_errors = Some(GovTalkErrors {
            error: Error {
                raised_by: "Gateway".to_string(),
                number: self.params.error_number.clone(),
                r#type: self.params.error_type.clone(),
                text: self.params.error_text.clone(),
                location: Some(self.params.error_location.clone()),
            },
        });
        let mut header = build_header(&self.params);
        header.message_details.response_endpoint = Some(ResponseEndpoint {
            poll_interval: Some(self.params.poll_interval.clone()),
            value: Some(self.params.response_endpoint.clone()),
        });
        Ok(GovTalkEnvelope {
            envelope_version: "2.0".to_string(),
            header,
            govtalk_details: details,
            body: Body {
                ir_envelope: None,
                success_response: None,
            },
        })
    }
}

// --- Submission Response ---

pub struct GovTalkSubmissionResponse {
    pub params: GovTalkParams,
}

impl GovTalkSubmissionResponse {
    pub fn new(params: GovTalkParams) -> Self {
        Self { params }
    }
}

impl Serialize for GovTalkSubmissionResponse {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let envelope = self
            .create_message()
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        envelope.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GovTalkSubmissionResponse {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let envelope = GovTalkEnvelope::deserialize(deserializer)?;
        let params = params_from_envelope(&envelope).map_err(serde::de::Error::custom)?;
        Ok(Self { params })
    }
}

impl Message for GovTalkSubmissionResponse {
    fn create_message(&self) -> Result<GovTalkEnvelope> {
        let mut header = build_header(&self.params);
        header.message_details.response_endpoint = Some(ResponseEndpoint {
            poll_interval: Some(self.params.poll_interval.clone()),
            value: Some(self.params.response_endpoint.clone()),
        });
        Ok(GovTalkEnvelope {
            envelope_version: "2.0".to_string(),
            header,
            govtalk_details: build_govtalk_details(&self.params),
            body: Body {
                ir_envelope: None,
                success_response: Some(SuccessResponse {
                    message: self.params.success_response_message.clone(),
                }),
            },
        })
    }
}

// --- Delete Request ---

pub struct GovTalkDeleteRequest {
    pub params: GovTalkParams,
}

impl GovTalkDeleteRequest {
    pub fn new(params: GovTalkParams) -> Self {
        Self { params }
    }
}

impl Serialize for GovTalkDeleteRequest {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let envelope = self
            .create_message()
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        envelope.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GovTalkDeleteRequest {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let envelope = GovTalkEnvelope::deserialize(deserializer)?;
        let params = params_from_envelope(&envelope).map_err(serde::de::Error::custom)?;
        Ok(Self { params })
    }
}

impl Message for GovTalkDeleteRequest {
    fn create_message(&self) -> Result<GovTalkEnvelope> {
        Ok(GovTalkEnvelope {
            envelope_version: "2.0".to_string(),
            header: build_header(&self.params),
            govtalk_details: build_govtalk_details(&self.params),
            body: Body {
                ir_envelope: None,
                success_response: None,
            },
        })
    }
}

// --- Delete Response ---

pub struct GovTalkDeleteResponse {
    pub params: GovTalkParams,
}

impl GovTalkDeleteResponse {
    pub fn new(params: GovTalkParams) -> Self {
        Self { params }
    }
}

impl Serialize for GovTalkDeleteResponse {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let envelope = self
            .create_message()
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        envelope.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GovTalkDeleteResponse {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let envelope = GovTalkEnvelope::deserialize(deserializer)?;
        let params = params_from_envelope(&envelope).map_err(serde::de::Error::custom)?;
        Ok(Self { params })
    }
}

impl Message for GovTalkDeleteResponse {
    fn create_message(&self) -> Result<GovTalkEnvelope> {
        let mut header = build_header(&self.params);
        header.message_details.response_endpoint = Some(ResponseEndpoint {
            poll_interval: Some(self.params.poll_interval.clone()),
            value: Some(self.params.response_endpoint.clone()),
        });
        Ok(GovTalkEnvelope {
            envelope_version: "2.0".to_string(),
            header,
            govtalk_details: build_govtalk_details(&self.params),
            body: Body {
                ir_envelope: None,
                success_response: None,
            },
        })
    }
}

// ============================================================================
// Decoder Factory
// ============================================================================

pub fn decode_govtalk_message(xml: &str) -> Result<GovTalkMessage> {
    let envelope: GovTalkEnvelope = quick_xml::de::from_str(xml)
        .map_err(|e| Ct600Error::XmlError {
            message: format!("Failed to parse XML: {}", e),
        })?;

    let function = envelope.header.message_details.function.clone();
    let qualifier = envelope.header.message_details.qualifier.clone();
    let params = extract_params(&envelope)?;

    match (function.as_str(), qualifier.as_str()) {
        ("submit", "request") => Ok(GovTalkMessage::SubmissionRequest(
            GovTalkSubmissionRequest { params },
        )),
        ("submit", "acknowledgement") => Ok(GovTalkMessage::SubmissionAcknowledgement(
            GovTalkSubmissionAcknowledgement { params },
        )),
        ("submit", "poll") => Ok(GovTalkMessage::SubmissionPoll(GovTalkSubmissionPoll {
            params,
        })),
        ("submit", "error") => Ok(GovTalkMessage::SubmissionError(GovTalkSubmissionError {
            params,
        })),
        ("submit", "response") => Ok(GovTalkMessage::SubmissionResponse(
            GovTalkSubmissionResponse { params },
        )),
        ("delete", "request") => Ok(GovTalkMessage::DeleteRequest(GovTalkDeleteRequest {
            params,
        })),
        ("delete", "response") => Ok(GovTalkMessage::DeleteResponse(GovTalkDeleteResponse {
            params,
        })),
        _ => Err(Ct600Error::XmlError {
            message: format!(
                "Unknown message type: function={}, qualifier={}",
                function, qualifier
            ),
        }),
    }
}

pub enum GovTalkMessage {
    SubmissionRequest(GovTalkSubmissionRequest),
    SubmissionAcknowledgement(GovTalkSubmissionAcknowledgement),
    SubmissionPoll(GovTalkSubmissionPoll),
    SubmissionError(GovTalkSubmissionError),
    SubmissionResponse(GovTalkSubmissionResponse),
    DeleteRequest(GovTalkDeleteRequest),
    DeleteResponse(GovTalkDeleteResponse),
}

fn extract_params(envelope: &GovTalkEnvelope) -> Result<GovTalkParams> {
    params_from_envelope(envelope)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submission_request_serialization() {
        let params = GovTalkParams {
            class: "HMRC-CT-CT600-TIL".to_string(),
            function: "submit".to_string(),
            qualifier: "request".to_string(),
            username: "testuser".to_string(),
            password: "testpass".to_string(),
            vendor_id: "12345".to_string(),
            tax_reference: "1234567890".to_string(),
            gateway_test: "1".to_string(),
            software: "CT600-RS".to_string(),
            software_version: "1.0.0".to_string(),
            irmark: "test-irmark".to_string(),
            ..Default::default()
        };

        let msg = GovTalkSubmissionRequest::new(params);
        let xml = msg.to_xml().unwrap();

        assert!(xml.contains("GovTalkMessage"));
        assert!(xml.contains("HMRC-CT-CT600-TIL"));
        assert!(xml.contains("testuser"));
        assert!(xml.contains("test-irmark"));

        let decoded = decode_govtalk_message(&xml).unwrap();
        let decoded_params = match &decoded {
            GovTalkMessage::SubmissionRequest(req) => &req.params,
            _ => panic!("Wrong message type"),
        };

        assert_eq!(decoded_params.username, "testuser");
        assert_eq!(decoded_params.class, "HMRC-CT-CT600-TIL");
    }

    #[test]
    fn test_error_message_serialization() {
        let params = GovTalkParams {
            class: "HMRC-CT-CT600-TIL".to_string(),
            function: "submit".to_string(),
            qualifier: "error".to_string(),
            error_number: "123".to_string(),
            error_type: "Validation".to_string(),
            error_text: "Invalid field".to_string(),
            error_location: "/Body/Field".to_string(),
            ..Default::default()
        };

        let msg = GovTalkSubmissionError::new(params);
        let xml = msg.to_xml().unwrap();

        assert!(xml.contains("GovTalkErrors"));
        assert!(xml.contains("123"));
        assert!(xml.contains("Invalid field"));
    }

    #[test]
    fn test_date_conversion() {
        let date_str = "15 January 2024";
        let iso = <GovTalkSubmissionRequest as Message>::to_iso_date(date_str).unwrap();
        assert_eq!(iso, "2024-01-15");
    }

    #[test]
    fn test_poll_message() {
        let params = GovTalkParams {
            class: "HMRC-CT-CT600-TIL".to_string(),
            function: "submit".to_string(),
            qualifier: "poll".to_string(),
            correlation_id: "test-123".to_string(),
            ..Default::default()
        };

        let msg = GovTalkSubmissionPoll::new(params);
        let xml = msg.to_xml().unwrap();

        assert!(xml.contains("poll"));
        assert!(xml.contains("test-123"));
    }
}
