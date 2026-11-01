//! Process-wide tracing setup.
//!
//! Default output: pretty logs in debug builds, JSON in release builds.
//! Distributed tracing is opt-in: build with the `otlp` feature and set
//! `OTEL_EXPORTER_OTLP_ENDPOINT`.

use tracing_subscriber::EnvFilter;

#[derive(Debug, thiserror::Error)]
#[error("telemetry initialization failed: {0}")]
pub struct TelemetryError(String);

/// Install the process-wide tracing subscriber. Call exactly once, at
/// process start (a second call panics: the global default is taken).
pub fn init(service_name: &str) -> Result<(), TelemetryError> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    #[cfg(feature = "otlp")]
    if let Some(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .filter(|value| !value.is_empty())
    {
        init_with_otlp(service_name, &endpoint, filter)?;
        return Ok(());
    }

    #[cfg(not(feature = "otlp"))]
    let _ = service_name;

    use tracing_subscriber::fmt::format::FmtSpan;
    if cfg!(debug_assertions) {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_span_events(FmtSpan::CLOSE)
            .init();
    } else {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .flatten_event(true)
            .init();
    }
    Ok(())
}

#[cfg(feature = "otlp")]
fn init_with_otlp(
    service_name: &str,
    endpoint: &str,
    filter: EnvFilter,
) -> Result<(), TelemetryError> {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| TelemetryError(e.to_string()))?;
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();
    let otel_layer =
        tracing_opentelemetry::layer().with_tracer(provider.tracer(service_name.to_owned()));

    tracing_subscriber::registry()
        .with(filter)
        .with(otel_layer)
        .with(tracing_subscriber::fmt::layer())
        .init();
    Ok(())
}
