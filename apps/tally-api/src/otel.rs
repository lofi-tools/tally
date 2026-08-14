//! Observability bootstrap (tracing: fastrace + logforth + the tokio-tracing
//! compatibility layer).
//!
//! Three layers, wired once at startup by [`init`]:
//!
//! 1. **tracing** stays the instrumentation API (handlers, tower-http,
//!    `db_log`). The subscriber is a [`Registry`] with an [`EnvFilter`] (the
//!    existing `RUST_LOG` semantics, which also gates what fastrace captures)
//!    plus [`fastrace_tracing::FastraceCompatLayer`] — the tokio-tracing ↔
//!    fastrace compatibility layer, so existing tracing spans/events become
//!    fastrace spans/events.
//! 2. **logforth** is the log sink. tracing events reach it via tracing's
//!    `log` feature; logforth filters with `RUST_LOG` and stamps
//!    `trace_id`/`span_id` (from the per-request fastrace local parent set in
//!    `lib.rs`, where trace id == `x-request-id`) via [`FastraceDiagnostic`].
//! 3. **fastrace** collects the spans and reports them: to OpenTelemetry
//!    (OTLP HTTP/protobuf) when `OTEL_EXPORTER_OTLP_ENDPOINT` is set — e.g.
//!    Traceway — and to a [`ConsoleReporter`] (stderr) otherwise, so local
//!    runs stay visible.
//!
//! The standard OpenTelemetry env vars are honoured: `OTEL_SERVICE_NAME`,
//! `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_HEADERS` (e.g.
//! `Authorization=Bearer …`), `OTEL_EXPORTER_OTLP_PROTOCOL` and the timeout.

use std::borrow::Cow;
use std::time::Duration;

use fastrace::collector::{Config, ConsoleReporter};
use fastrace_opentelemetry::OpenTelemetryReporter;
use opentelemetry::InstrumentationScope;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::Resource;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;

/// `OTEL_EXPORTER_OTLP_ENDPOINT` — when set, spans are exported there (OTLP
/// HTTP/protobuf); when unset, fastrace falls back to printing to stderr.
const OTEL_ENDPOINT_ENV: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
/// `OTEL_SERVICE_NAME` — the `service.name` resource attribute.
const OTEL_SERVICE_NAME_ENV: &str = "OTEL_SERVICE_NAME";
/// Default service name when `OTEL_SERVICE_NAME` is unset.
const DEFAULT_SERVICE_NAME: &str = "tally-api";
/// Log filter when `RUST_LOG` is unset (matches the pre-logforth default).
const DEFAULT_LOG_FILTER: &str = "tally_api=info,tower_http=info";

/// Initialize tracing + logs + the fastrace reporter. Call once, early in
/// `main`, before any request is served or span is created.
pub fn init() {
    // tracing: spans/events → fastrace through the tokio-tracing compat
    // layer; EnvFilter keeps the RUST_LOG semantics (and gates what fastrace
    // captures).
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    let subscriber = tracing_subscriber::Registry::default()
        .with(env_filter)
        .with(fastrace_tracing::FastraceCompatLayer::new());
    tracing::subscriber::set_global_default(subscriber)
        .expect("set tracing global default (otel::init is called once)");

    // logs: logforth. tracing events arrive via tracing's `log` feature;
    // RUST_LOG filters; FastraceDiagnostic stamps trace_id/span_id.
    logforth::starter_log::builder()
        .dispatch(|d| {
            d.filter(
                logforth::filter::rustlog::RustLogFilterBuilder::from_default_env_or(
                    DEFAULT_LOG_FILTER,
                )
                .build(),
            )
            // tracing's log-always feature also emits span-lifecycle records
            // ("++ span; fields" / "-- span; fields") — drop them here.
            .filter(SpanLifecycleFilter)
            .diagnostic(logforth::diagnostic::FastraceDiagnostic::default())
            .append(logforth::append::Stdout::default().with_layout(logforth::layout::TextLayout::default()))
        })
        .apply();

    // fastrace reporter: OTel when configured, console otherwise.
    let endpoint = env_or(OTEL_ENDPOINT_ENV, "");
    let config = Config::default().report_interval(Duration::from_secs(1));
    if endpoint.is_empty() {
        tracing::info!("OTEL_EXPORTER_OTLP_ENDPOINT unset — fastrace spans go to stderr (ConsoleReporter)");
        fastrace::set_reporter(ConsoleReporter, config);
    } else {
        let reporter = build_otel_reporter();
        tracing::info!(
            endpoint = %endpoint,
            service = %env_or(OTEL_SERVICE_NAME_ENV, DEFAULT_SERVICE_NAME),
            "fastrace spans → OpenTelemetry (OTLP http/protobuf)"
        );
        fastrace::set_reporter(reporter, config);
    }
}

/// The OpenTelemetry reporter: OTLP over HTTP/protobuf. The exporter is
/// configured purely from the standard `OTEL_EXPORTER_OTLP_*` env vars —
/// `OTEL_EXPORTER_OTLP_ENDPOINT` (with the `/v1/traces` signal path appended
/// automatically), `OTEL_EXPORTER_OTLP_HEADERS` (e.g.
/// `Authorization=Bearer …`), `OTEL_EXPORTER_OTLP_PROTOCOL`, timeout. Note:
/// passing `with_endpoint` programmatically would bypass that env resolution
/// and use the URL verbatim (no `/v1/traces`), so it is deliberately omitted.
fn build_otel_reporter() -> OpenTelemetryReporter {
    let exporter = SpanExporter::builder()
        .with_http()
        .build()
        .expect("build OTLP span exporter");
    let resource = Resource::builder()
        .with_service_name(env_or(OTEL_SERVICE_NAME_ENV, DEFAULT_SERVICE_NAME))
        .build();
    let scope = InstrumentationScope::builder(DEFAULT_SERVICE_NAME)
        .with_version(env!("CARGO_PKG_VERSION"))
        .build();
    OpenTelemetryReporter::new(exporter, Cow::Owned(resource), scope)
}

/// Env var or default (empty counts as unset).
fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Drops tracing's span-lifecycle records — the `++ span; fields` /
/// `-- span; fields` lines tracing's `log`/`log-always` feature emits at span
/// creation/exit. They exist for subscribers that render span context; with
/// logforth the same fields ride the actual events (e.g. the access-log
/// line), so these would be pure noise.
#[derive(Debug, Default)]
struct SpanLifecycleFilter;

impl logforth::filter::Filter for SpanLifecycleFilter {
    fn enabled(
        &self,
        _criteria: &logforth::record::FilterCriteria,
        _diags: &[Box<dyn logforth::diagnostic::Diagnostic>],
    ) -> logforth::filter::FilterResult {
        // Can't decide from metadata alone (same target/level as real
        // records); the per-record `matches` does the real work.
        logforth::filter::FilterResult::Neutral
    }

    fn matches(
        &self,
        record: &logforth::record::Record,
        _diags: &[Box<dyn logforth::diagnostic::Diagnostic>],
    ) -> logforth::filter::FilterResult {
        let payload = format!("{}", record.payload());
        if payload.starts_with("++ ") || payload.starts_with("-- ") {
            logforth::filter::FilterResult::Reject
        } else {
            logforth::filter::FilterResult::Neutral
        }
    }
}
