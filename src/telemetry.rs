use crate::config::AppEnvironment;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initializes the global tracing subscriber based on the application environment.
///
/// - In `Production`: emits structured JSON logs with targets, thread IDs, and thread names.
/// - In `Development` / `Test`: emits human-readable pretty formatted logs.
pub fn init_tracing(environment: &AppEnvironment) {
    let _ = try_init_tracing(environment);
}

/// Attempts to initialize the global tracing subscriber.
/// Returns an error if a global default subscriber has already been set.
///
/// # Errors
/// Returns an error if setting the global default subscriber fails.
pub fn try_init_tracing(
    environment: &AppEnvironment,
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

            tracing_subscriber::registry()
                .with(env_filter)
                .with(formatting_layer)
                .try_init()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
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
        let _ = try_init_tracing(&AppEnvironment::Development);
    }
}
