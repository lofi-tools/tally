#![cfg(test)]
//! Test utilities for the ct600 crate.
//!
//! The company fixtures and offline test clients live in
//! `crate::companies_house::test_utils`; [`sample_values`] derives the CT600
//! form values from the shared sample tax computation and [`TestData`] hosts
//! the hardcoded fixtures for the HMRC client tests (a sample return and a
//! test config), so tests run with zero configuration on a fresh checkout.

use crate::clients::HmrcCorpTaxConfig;
use crate::clients::hmrc_corp_tax::CLASS_LIVE;
use crate::ct600_return::{
    CompanyInformation, Ct600Return, Declaration, EnvelopeConfig, FinancialYear,
    ReturnInfoSummary,
};
use crate::form::Ct600FormValues;
use crate::govtalk::{
    GovTalkDeleteResponse, GovTalkParams, GovTalkSubmissionAcknowledgement, GovTalkSubmissionError,
    GovTalkSubmissionResponse, Message,
};
use chrono::NaiveDate;

/// The CT600 form values derived from the shared sample tax computation
/// (`crate::companies_house::test_utils::TestData::sample_tax`).
pub fn sample_values() -> Ct600FormValues {
    Ct600FormValues::from_tax(&crate::companies_house::test_utils::TestData::sample_tax())
}

/// Hardcoded test data for the HMRC Corporation Tax client tests.
pub struct TestData;

impl TestData {
    /// A minimal but complete `Ct600Return` for message-building tests.
    pub fn sample_return() -> Ct600Return {
        Ct600Return {
            envelope: EnvelopeConfig::default(),
            contact: Default::default(),
            sender: "Company".to_string(),
            company: CompanyInformation {
                company_name: "Acme Ltd".to_string(),
                registration_number: "12345678".to_string(),
                reference: "8596148860".to_string(),
                company_type: 1,
                period_start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                period_end: NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            },
            return_info: ReturnInfoSummary {
                this_period_accounts: true,
                this_period_computations: true,
            },
            turnover: 11218.0,
            trading_profits: 748.0,
            net_trading_profits: 748.0,
            profits_before_other_deductions: 748.0,
            profits_before_charges_and_group_relief: 748.0,
            chargeable_profits: 748.0,
            fy1: FinancialYear {
                year: 2024,
                profit: 186.0,
                tax_rate: 19.0,
                tax: 35.34,
            },
            fy2: FinancialYear {
                year: 2025,
                profit: 562.0,
                tax_rate: 19.0,
                tax: 106.78,
            },
            corporation_tax: 142.12,
            net_corporation_tax_chargeable: 142.12,
            net_corporation_tax_liability: 142.12,
            tax_chargeable: 142.12,
            tax_payable: 142.12,
            tax_payable_including_restitution_tax: 142.12,
            sme_claim: true,
            rnd_enhanced_expenditure: Some(465.0),
            rnd_and_creative_enhanced_expenditure: Some(465.0),
            aia_capital_allowances: 591.0,
            payment_address_lines: vec!["1 High Street".to_string()],
            payment_recipient: "Acme Ltd".to_string(),
            payment_nominee_reference: "8596148860".to_string(),
            declaration: Declaration {
                name: Some("Jane Doe".to_string()),
                date: Some(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()),
                status: Some("Director".to_string()),
            },
            computation_document: Some("<html/>".to_string()),
            accounts_document: Some("<html/>".to_string()),
        }
    }

    /// A `HmrcCorpTaxConfig` with fixed test credentials.
    pub fn test_config() -> HmrcCorpTaxConfig {
        HmrcCorpTaxConfig::test_from_env()
            .with_username("testuser")
            .with_password("testpass")
            .with_vendor_id("1234")
    }
}

// ============================================================================
// An in-process GovTalk stub gateway
// ============================================================================

/// A minimal in-process stand-in for the HMRC Transaction Engine.
///
/// Accepts submission requests (replying with an acknowledgement), responds
/// to polls with either a success response or a gateway error (controlled by
/// [`Self::reject_polls`]), and counts delete requests.  The client is pointed
/// at it with `HmrcCorpTaxConfig::with_submission_url` / `with_poll_url`.
pub(crate) struct StubGateway {
    /// The base URL (`http://host:port`) of the stub.
    pub(crate) base: String,
    /// When set, polls are answered with a gateway error instead of a
    /// success response.
    pub(crate) reject_polls: std::sync::Arc<std::sync::atomic::AtomicBool>,
    deletions: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl StubGateway {
    /// Bind a listener on an ephemeral port and serve requests in the
    /// background.
    pub(crate) async fn spawn() -> Self {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind stub");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let reject_polls = std::sync::Arc::new(AtomicBool::new(false));
        let deletions = std::sync::Arc::new(AtomicUsize::new(0));
        let reject = reject_polls.clone();
        let del = deletions.clone();

        tokio::spawn(async move {
            loop {
                let (mut socket, _) = match listener.accept().await {
                    Ok(pair) => pair,
                    Err(_) => break,
                };
                let reject = reject.clone();
                let del = del.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 16384];
                    let n = match socket.read(&mut buf).await {
                        Ok(n) if n > 0 => n,
                        _ => return,
                    };
                    let request = String::from_utf8_lossy(&buf[..n]).to_string();
                    let response = route(&request, reject.load(Ordering::Relaxed), &del);
                    let _ = socket
                        .write_all(
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                response.len(),
                                response
                            )
                            .as_bytes(),
                        )
                        .await;
                });
            }
        });

        Self {
            base,
            reject_polls,
            deletions,
        }
    }

    /// Whether a delete request has been received.
    pub(crate) fn deleted(&self) -> bool {
        self.deletions.load(std::sync::atomic::Ordering::Relaxed) > 0
    }
}

/// Route a raw HTTP request body to a GovTalk response.
fn route(request: &str, reject_polls: bool, deletions: &std::sync::atomic::AtomicUsize) -> String {
    if request.contains("Qualifier>request") && request.contains("Function>submit") {
        // Submission request -> acknowledgement with a correlation ID.
        let params = GovTalkParams {
            class: CLASS_LIVE.to_string(),
            function: "submit".to_string(),
            qualifier: "acknowledgement".to_string(),
            correlation_id: "CORR-1".to_string(),
            response_endpoint: String::new(),
            poll_interval: "1".to_string(),
            ..Default::default()
        };
        GovTalkSubmissionAcknowledgement::new(params).to_xml().expect("ack")
    } else if request.contains("Qualifier>poll") {
        if reject_polls {
            let params = GovTalkParams {
                class: CLASS_LIVE.to_string(),
                function: "submit".to_string(),
                qualifier: "error".to_string(),
                correlation_id: "CORR-1".to_string(),
                error_number: "1001".to_string(),
                error_type: "business".to_string(),
                error_text: "Box 145 is invalid".to_string(),
                ..Default::default()
            };
            GovTalkSubmissionError::new(params).to_xml().expect("error")
        } else {
            let params = GovTalkParams {
                class: CLASS_LIVE.to_string(),
                function: "submit".to_string(),
                qualifier: "response".to_string(),
                correlation_id: "CORR-1".to_string(),
                success_response_message: "Submission processed successfully".to_string(),
                ..Default::default()
            };
            GovTalkSubmissionResponse::new(params).to_xml().expect("response")
        }
    } else if request.contains("Function>delete") {
        deletions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let params = GovTalkParams {
            class: CLASS_LIVE.to_string(),
            function: "delete".to_string(),
            qualifier: "response".to_string(),
            correlation_id: "CORR-1".to_string(),
            ..Default::default()
        };
        GovTalkDeleteResponse::new(params).to_xml().expect("delete")
    } else {
        String::new()
    }
}
