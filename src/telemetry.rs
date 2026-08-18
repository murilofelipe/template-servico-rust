use crate::config::AppEnvironment;
use opentelemetry::KeyValue;
use opentelemetry_sdk::{propagation::TraceContextPropagator, trace as sdktrace, Resource};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initializes the global tracing subscriber based on the application environment.
pub fn init_tracing(
    environment: &AppEnvironment,
    otel_service_name: Option<&str>,
    otlp_endpoint: Option<&str>,
) {
    let _ = try_init_tracing(environment, otel_service_name, otlp_endpoint);
}

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::trace::TracerProvider;

pub fn init_otel_tracer(
    service_name: &str,
    otlp_endpoint: &str,
) -> Result<sdktrace::Tracer, Box<dyn std::error::Error + Send + Sync>> {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_resource(Resource::new(vec![KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_NAME,
            service_name.to_string(),
        )]))
        .build();

    opentelemetry::global::set_tracer_provider(provider.clone());

    Ok(provider.tracer("template-servico-rust"))
}

/// Attempts to initialize the global tracing subscriber.
pub fn try_init_tracing(
    environment: &AppEnvironment,
    otel_service_name: Option<&str>,
    otlp_endpoint: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,template_servico_rust=debug,tower_http=info"));

    match environment {
        AppEnvironment::Production => {
            let formatting_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true);

            if let Some(endpoint) = otlp_endpoint {
                let name = otel_service_name.unwrap_or("template-servico-rust");
                let tracer = init_otel_tracer(name, endpoint)?;
                let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(formatting_layer)
                    .with(otel_layer)
                    .try_init()
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            } else {
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(formatting_layer)
                    .try_init()
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            }
        }
        AppEnvironment::Development | AppEnvironment::Test => {
            let formatting_layer = tracing_subscriber::fmt::layer().pretty().with_target(true);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(formatting_layer)
                .try_init()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::try_init_tracing;
    use crate::config::AppEnvironment;

    #[test]
    fn test_init_tracing_does_not_panic() {
        let _ = try_init_tracing(&AppEnvironment::Development, None, None);
    }

    #[test]
    fn test_init_tracing_production() {
        let _ = try_init_tracing(&AppEnvironment::Production, None, None);
    }

    #[test]
    fn test_init_tracing_test_env() {
        let _ = try_init_tracing(&AppEnvironment::Test, None, None);
    }
}
