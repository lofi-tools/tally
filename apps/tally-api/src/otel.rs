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
//! On top of the trace export, **every logforth record is also exported as an
//! OTLP log** to the same endpoint (`/v1/logs`) via [`OtelLogAppend`] — so the
//! full log stream lands in Traceway too: access logs, db queries, background
//! worker logs, startup lines, everything that passes the `RUST_LOG` filter,
//! including logs emitted outside any request span. Records emitted inside a
//! fastrace span carry the trace/span id as the OTel trace context, so
//! Traceway links them to their originating trace.
//!
//! The standard OpenTelemetry env vars are honoured: `OTEL_SERVICE_NAME`,
//! `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_HEADERS` (e.g.
//! `Authorization=Bearer …`), `OTEL_EXPORTER_OTLP_PROTOCOL` and the timeout.

use std::borrow::Cow;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use fastrace::collector::{Config, ConsoleReporter};
use fastrace_opentelemetry::OpenTelemetryReporter;
use logforth::kv::{KeyView, ValueView};
use logforth::record::{Level, Record};
use logforth::{Diagnostic, Error as LogforthError};
use opentelemetry::logs::{
    AnyValue, LogRecord as _, Logger as _, LoggerProvider as _, Severity,
};
use opentelemetry::trace::{SpanId, TraceFlags, TraceId};
use opentelemetry::InstrumentationScope;
use opentelemetry_otlp::{LogExporter, SpanExporter};
use opentelemetry_sdk::logs::{
    BatchConfigBuilder, BatchLogProcessor, SdkLogger, SdkLoggerProvider,
};
use opentelemetry_sdk::Resource;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;

/// `OTEL_EXPORTER_OTLP_ENDPOINT` — when set, spans and logs are exported there
/// (OTLP HTTP/protobuf); when unset, fastrace falls back to printing to stderr
/// and logs go only to stdout.
const OTEL_ENDPOINT_ENV: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
/// `OTEL_SERVICE_NAME` — the `service.name` resource attribute.
const OTEL_SERVICE_NAME_ENV: &str = "OTEL_SERVICE_NAME";
/// Default service name when `OTEL_SERVICE_NAME` is unset.
const DEFAULT_SERVICE_NAME: &str = "tally-api";
/// Log filter when `RUST_LOG` is unset (matches the pre-logforth default).
/// `fastrace_opentelemetry=error` is included so a failed OTLP export (bad
/// endpoint, 401, network) shows up in the API logs instead of dropping
/// spans silently — without a matching directive (or a bare default level)
/// logforth rejects records from unknown targets outright.
const DEFAULT_LOG_FILTER: &str =
    "tally_api=info,tower_http=info,fastrace_opentelemetry=error";

/// The logs `SdkLoggerProvider` (batch exporter), kept so `flush_logs()` can
/// drain pending records at shutdown. Set once by [`init`].
static OTEL_LOG_PROVIDER: OnceLock<Option<SdkLoggerProvider>> = OnceLock::new();

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

    // Decide the OTel path once: when the endpoint is set, every log record is
    // also exported as an OTLP log (the append is moved into the dispatch
    // below, so it must be built first).
    let endpoint = env_or(OTEL_ENDPOINT_ENV, "");
    let otel_log_append = if endpoint.is_empty() {
        None
    } else {
        warn_on_suspicious_otel_headers();
        Some(build_otel_log_append())
    };

    // logs: logforth. tracing events arrive via tracing's `log` feature;
    // RUST_LOG filters; FastraceDiagnostic stamps trace_id/span_id; the OTel
    // append (when configured) forwards every record to the OTLP /v1/logs
    // endpoint.
    logforth::starter_log::builder()
        .dispatch(move |d| {
            let d = d
                .filter(
                    logforth::filter::rustlog::RustLogFilterBuilder::from_default_env_or(
                        DEFAULT_LOG_FILTER,
                    )
                    .build(),
                )
                // tracing's log-always feature also emits span-lifecycle
                // records ("++ span; fields" / "-- span; fields") — drop them
                // here.
                .filter(SpanLifecycleFilter)
                .diagnostic(logforth::diagnostic::FastraceDiagnostic::default())
                .append(
                    logforth::append::Stdout::default()
                        .with_layout(logforth::layout::TextLayout::default()),
                );
            match otel_log_append {
                Some(append) => d.append(append),
                None => d,
            }
        })
        .apply();

    // fastrace reporter: OTel when configured, console otherwise.
    let config = Config::default().report_interval(Duration::from_secs(1));
    if endpoint.is_empty() {
        tracing::info!("OTEL_EXPORTER_OTLP_ENDPOINT unset — fastrace spans go to stderr (ConsoleReporter)");
        fastrace::set_reporter(ConsoleReporter, config);
    } else {
        let reporter = build_otel_reporter();
        tracing::info!(
            endpoint = %endpoint,
            service = %env_or(OTEL_SERVICE_NAME_ENV, DEFAULT_SERVICE_NAME),
            otlp_headers_set = !std::env::var("OTEL_EXPORTER_OTLP_HEADERS")
                .map(|v| v.is_empty())
                .unwrap_or(true),
            "fastrace spans + logs → OpenTelemetry (OTLP http/protobuf)"
        );
        fastrace::set_reporter(reporter, config);
    }
}

/// Flush pending OTLP logs (the batch exporter holds up to a second of
/// records). Call at shutdown, after `fastrace::flush()`.
pub fn flush_logs() {
    if let Some(Some(provider)) = OTEL_LOG_PROVIDER.get() {
        let _ = provider.force_flush();
    }
}

/// The OpenTelemetry trace reporter: OTLP over HTTP/protobuf. The exporter is
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

/// The logforth append that forwards every log record to the OTLP `/v1/logs`
/// endpoint (same env resolution as the trace exporter). Records emitted
/// inside a fastrace span (e.g. during a request) carry the trace/span id as
/// the OTel trace context, which is how Traceway links logs to their traces.
fn build_otel_log_append() -> OtelLogAppend {
    let exporter = LogExporter::builder()
        .with_http()
        .build()
        .expect("build OTLP log exporter");
    let resource = Resource::builder()
        .with_service_name(env_or(OTEL_SERVICE_NAME_ENV, DEFAULT_SERVICE_NAME))
        .build();
    // Batch processor on its own thread: appends are cheap (enqueue), export
    // happens in the background every second (matching fastrace's interval),
    // and `flush_logs()` drains at shutdown.
    let provider = SdkLoggerProvider::builder()
        .with_log_processor(
            BatchLogProcessor::builder(exporter)
                .with_batch_config(
                    BatchConfigBuilder::default()
                        .with_scheduled_delay(Duration::from_secs(1))
                        .with_max_queue_size(10_000)
                        .build(),
                )
                .build(),
        )
        .with_resource(resource)
        .build();
    let logger = provider.logger(DEFAULT_SERVICE_NAME);
    OTEL_LOG_PROVIDER
        .set(Some(provider))
        .expect("set OTEL_LOG_PROVIDER (otel::init is called once)");
    OtelLogAppend { logger }
}

/// Best-effort startup check for the classic `Authorization=Bearer` env
/// truncation: a value with a space (`Bearer <token>`) that got word-split
/// when exported lands in the process env as `Authorization=Bearer` plus the
/// token as a stray separate variable — Traceway then 401s every export and
/// spans/logs are dropped. Warn loudly instead of failing silently.
fn warn_on_suspicious_otel_headers() {
    let Ok(headers) = std::env::var("OTEL_EXPORTER_OTLP_HEADERS") else {
        return;
    };
    for pair in headers.split(',').map(str::trim) {
        let Some((key, value)) = pair.split_once('=') else { continue };
        if key.trim().eq_ignore_ascii_case("authorization")
            && value.trim().eq_ignore_ascii_case("bearer")
        {
            tracing::warn!(
                "OTEL_EXPORTER_OTLP_HEADERS Authorization is a bare 'Bearer' with no token — \
                 the token was likely split off at the space when exported; \
                 Traceway will reject every export (401) and spans/logs will be dropped"
            );
            return;
        }
    }
}

/// Env var or default (empty counts as unset).
fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// The logforth append that exports every log record to OpenTelemetry as an
/// OTLP log (`/v1/logs`). Deliberately a small custom append rather than
/// `logforth::append::opentelemetry`: that one only adds the diagnostic's
/// `trace_id`/`span_id` as string *attributes*, but Traceway links logs to
/// traces via the OTel trace-context fields — so we set those explicitly.
#[derive(Debug)]
struct OtelLogAppend {
    logger: SdkLogger,
}

impl logforth::Append for OtelLogAppend {
    fn append(
        &self,
        record: &Record<'_>,
        diags: &[Box<dyn Diagnostic>],
    ) -> Result<(), LogforthError> {
        let now = SystemTime::now();
        let mut log_record = self.logger.create_log_record();
        log_record.set_timestamp(now);
        log_record.set_observed_timestamp(now);
        log_record.set_severity_number(log_level_to_otel_severity(record.level()));
        log_record.set_severity_text(record.level().name());
        log_record.set_target(record.target().to_owned());
        if let Some(payload) = record.payload_static() {
            log_record.set_body(AnyValue::from(payload));
        } else {
            log_record.set_body(AnyValue::from(record.payload().to_string()));
        }
        if let Some(module_path) = record.module_path_static() {
            log_record.add_attribute("code.namespace", module_path);
        } else if let Some(module_path) = record.module_path() {
            log_record.add_attribute("code.namespace", module_path.to_owned());
        }
        if let Some(file) = record.file_static() {
            log_record.add_attribute("code.filepath", file);
        } else if let Some(file) = record.file() {
            log_record.add_attribute("code.filepath", file.to_owned());
        }
        if let Some(line) = record.line() {
            log_record.add_attribute("code.lineno", line);
        }
        if let Some(column) = record.column() {
            log_record.add_attribute("code.column", column);
        }

        // Record KV pairs (the tracing fields — method/uri/request_id on the
        // access log, etc.) plus the diagnostic KVs (FastraceDiagnostic's
        // trace_id/span_id/sampled) become log attributes.
        let mut extractor = KvExtractor { record: &mut log_record };
        record.key_values().visit(&mut extractor)?;
        for diag in diags {
            diag.visit(&mut extractor)?;
        }

        // OTel trace context: hex trace/span id from the FastraceDiagnostic —
        // Traceway's Trace ID / Span ID columns (and the log↔trace link) come
        // from these fields, not from attributes.
        if let Some((trace_id, span_id)) = trace_context_from_diags(diags) {
            log_record.set_trace_context(trace_id, span_id, Some(TraceFlags::SAMPLED));
        }

        self.logger.emit(log_record);
        Ok(())
    }

    fn flush(&self) -> Result<(), LogforthError> {
        Ok(())
    }
}

impl Drop for OtelLogAppend {
    fn drop(&mut self) {
        // The append outlives the provider only in tests; the real drain
        // happens in `flush_logs()` at shutdown.
    }
}

fn log_level_to_otel_severity(level: Level) -> Severity {
    match level {
        Level::Trace => Severity::Trace,
        Level::Trace2 => Severity::Trace2,
        Level::Trace3 => Severity::Trace3,
        Level::Trace4 => Severity::Trace4,
        Level::Debug => Severity::Debug,
        Level::Debug2 => Severity::Debug2,
        Level::Debug3 => Severity::Debug3,
        Level::Debug4 => Severity::Debug4,
        Level::Info => Severity::Info,
        Level::Info2 => Severity::Info2,
        Level::Info3 => Severity::Info3,
        Level::Info4 => Severity::Info4,
        Level::Warn => Severity::Warn,
        Level::Warn2 => Severity::Warn2,
        Level::Warn3 => Severity::Warn3,
        Level::Warn4 => Severity::Warn4,
        Level::Error => Severity::Error,
        Level::Error2 => Severity::Error2,
        Level::Error3 => Severity::Error3,
        Level::Error4 => Severity::Error4,
        Level::Fatal => Severity::Fatal,
        Level::Fatal2 => Severity::Fatal2,
        Level::Fatal3 => Severity::Fatal3,
        Level::Fatal4 => Severity::Fatal4,
    }
}

/// Pulls the OTel trace context out of the diagnostics: the
/// [`logforth::diagnostic::FastraceDiagnostic`] visits `trace_id` (32 hex
/// chars) and `span_id` (16 hex chars) when a fastrace local parent is set.
fn trace_context_from_diags(diags: &[Box<dyn Diagnostic>]) -> Option<(TraceId, SpanId)> {
    let mut extractor = TraceContextExtractor::default();
    for diag in diags {
        let _ = diag.visit(&mut extractor);
    }
    let trace_id = TraceId::from_hex(extractor.trace_id.as_deref()?).ok()?;
    let span_id = SpanId::from_hex(extractor.span_id.as_deref()?).ok()?;
    Some((trace_id, span_id))
}

#[derive(Default)]
struct TraceContextExtractor {
    trace_id: Option<String>,
    span_id: Option<String>,
}

impl logforth::kv::Visitor for TraceContextExtractor {
    fn visit(&mut self, key: KeyView<'_>, value: ValueView<'_>) -> Result<(), LogforthError> {
        match key.as_str() {
            "trace_id" => self.trace_id = value.to_str().map(str::to_owned),
            "span_id" => self.span_id = value.to_str().map(str::to_owned),
            _ => {}
        }
        Ok(())
    }
}

/// Copies the record + diagnostic KV pairs onto the OTel log record as
/// attributes (same conversion as logforth-append-opentelemetry, Apache-2.0).
struct KvExtractor<'a> {
    record: &'a mut opentelemetry_sdk::logs::SdkLogRecord,
}

impl logforth::kv::Visitor for KvExtractor<'_> {
    fn visit(&mut self, key: KeyView<'_>, value: ValueView<'_>) -> Result<(), LogforthError> {
        self.record
            .add_attribute(opentelemetry::Key::from(key.to_cow()), value_to_any_value(value));
        Ok(())
    }
}

fn value_to_any_value(value: ValueView<'_>) -> AnyValue {
    match value {
        ValueView::None => AnyValue::String("null".into()),
        ValueView::BorrowedStr(v) => AnyValue::String(v.to_string().into()),
        ValueView::StaticStr(v) => AnyValue::String(v.into()),
        ValueView::Char(v) => AnyValue::String(v.to_string().into()),
        ValueView::Debug(v) => AnyValue::String(v.to_string().into()),
        ValueView::Display(v) => AnyValue::String(v.to_string().into()),
        ValueView::Bytes(v) => AnyValue::Bytes(Box::new(v.to_vec())),
        ValueView::Bool(v) => AnyValue::Boolean(v),
        ValueView::I64(v) => AnyValue::Int(v),
        ValueView::F64(v) => AnyValue::Double(v),
        ValueView::U64(v) => {
            if let Ok(i) = i64::try_from(v) {
                AnyValue::Int(i)
            } else {
                AnyValue::String(v.to_string().into())
            }
        }
        ValueView::I128(v) => {
            if let Ok(i) = i64::try_from(v) {
                AnyValue::Int(i)
            } else {
                AnyValue::String(v.to_string().into())
            }
        }
        ValueView::U128(v) => {
            if let Ok(i) = i64::try_from(v) {
                AnyValue::Int(i)
            } else {
                AnyValue::String(v.to_string().into())
            }
        }
        ValueView::List(v) => AnyValue::ListAny(Box::new(
            v.iter().map(value_to_any_value).collect(),
        )),
        ValueView::Map(v) => AnyValue::Map(Box::new(
            v.iter()
                .map(|(k, v)| (opentelemetry::Key::from(k.to_cow()), value_to_any_value(v)))
                .collect(),
        )),
        // `ValueView` is `#[non_exhaustive]` — keep future variants stringified.
        _ => AnyValue::String(value.to_string().into()),
    }
}

/// Drops tracing's span-lifecycle records — the `++ span; fields` /
/// `-- span; fields` lines tracing's `log`/`log-always` feature emits at span
/// creation/exit. They exist for subscribers that render span context; with
/// logforth the same fields ride the actual events (e.g. the access-log
/// line), so these would be pure noise (in stdout and in Traceway).
#[derive(Debug, Default)]
struct SpanLifecycleFilter;

impl logforth::filter::Filter for SpanLifecycleFilter {
    fn enabled(
        &self,
        _criteria: &logforth::record::FilterCriteria,
        _diags: &[Box<dyn Diagnostic>],
    ) -> logforth::filter::FilterResult {
        // Can't decide from metadata alone (same target/level as real
        // records); the per-record `matches` does the real work.
        logforth::filter::FilterResult::Neutral
    }

    fn matches(
        &self,
        record: &Record<'_>,
        _diags: &[Box<dyn Diagnostic>],
    ) -> logforth::filter::FilterResult {
        let payload = format!("{}", record.payload());
        if payload.starts_with("++ ") || payload.starts_with("-- ") {
            logforth::filter::FilterResult::Reject
        } else {
            logforth::filter::FilterResult::Neutral
        }
    }
}
