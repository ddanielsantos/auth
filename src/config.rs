pub mod database {
    use crate::config::env::Env;
    use sqlx::PgPool;
    use sqlx::pool::PoolOptions;
    use std::time::Duration;

    /// Creates and returns a PostgreSQL connection pool.
    ///
    /// Accepts an optional database URL; if not provided, uses the DATABASE_URL environment variable.
    /// Configures a pool with a maximum of 20 connections and a 3-second acquisition timeout.
    pub async fn get_connection_pool(database_url: Option<String>) -> Result<PgPool, sqlx::Error> {
        let uri = database_url.unwrap_or_else(get_default_database_url);

        PoolOptions::new()
            .max_connections(20)
            .acquire_timeout(Duration::from_secs(3))
            .connect(uri.as_str())
            .await
    }

    fn get_default_database_url() -> String {
        let Env { database_url, .. } = Env::new();

        database_url
    }
}

mod env;

pub mod tracing {
    use tokio::signal;
    use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, HttpMakeClassifier, TraceLayer};
    use tracing::Level;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    /// Initializes the tracing subscriber for structured logging.
    ///
    /// Sets up logging with configurable levels via the RUST_LOG environment variable,
    /// defaulting to debug level for the application and tower_http, with trace level for axum.
    /// Logs are formatted without timestamps.
    pub fn init_tracing() {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    format!("{}=debug,tower_http=debug,axum=trace", env!("CARGO_CRATE_NAME")).into()
                }),
            )
            .with(tracing_subscriber::fmt::layer().without_time())
            .init()
    }

    /// Waits for a shutdown signal (Ctrl+C or SIGTERM).
    ///
    /// On Unix systems, listens for both SIGINT (Ctrl+C) and SIGTERM signals.
    /// On other systems, only responds to Ctrl+C. Blocks until one of these signals is received.
    pub async fn shutdown_signal() {
        let ctrl_c = async {
            signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }

    /// Creates and returns a trace layer for HTTP request/response logging.
    ///
    /// Configures tracing with INFO level for both request spans and response events,
    /// providing visibility into HTTP traffic through structured logging.
    pub fn get_trace_layer() -> TraceLayer<HttpMakeClassifier> {
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_response(DefaultOnResponse::new().level(Level::INFO))
    }
}

pub mod net {
    use axum::http::Method;
    use lazy_limit::{Duration, RuleConfig, init_rate_limiter};
    use tower_http::cors;
    use tower_http::cors::{AllowHeaders, CorsLayer};

    /// Creates and returns a CORS layer configured to allow cross-origin requests.
    ///
    /// The layer permits GET, POST, and OPTIONS HTTP methods, allows any headers,
    /// and accepts requests from any origin.
    pub fn get_cors_layer() -> CorsLayer {
        CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(AllowHeaders::any())
            .allow_origin(cors::Any)
    }

    /// Initializes rate limiting rules for the application.
    ///
    /// Configures per-route rate limits with a default of 10 requests per minute.
    ///
    /// Auth endpoints allow 5 requests per 15 minutes, while read/write operations have
    /// customized limits.
    ///
    /// Maximum memory usage is capped at 64 MB.
    pub async fn init_rate_limiting() {
        init_rate_limiter!(
            default: RuleConfig::new(Duration::minutes(1), 10),
            max_memory: Some(64 * 1024 * 1024),
            routes: [
                // auth
                ("/admin/register", RuleConfig::new(Duration::minutes(15), 5)),
                ("/admin/login", RuleConfig::new(Duration::minutes(15), 5)),
                ("/auth/register", RuleConfig::new(Duration::minutes(15), 5)),
                ("/auth/login", RuleConfig::new(Duration::minutes(15), 5)),

                // write
                ("/admin/organizations", RuleConfig::new(Duration::minutes(1), 10)),
                ("/admin/projects", RuleConfig::new(Duration::minutes(1), 10)),
                ("/admin/applications", RuleConfig::new(Duration::minutes(1), 10)),

                // read
                ("/api/me", RuleConfig::new(Duration::minutes(1), 100))
            ]
        )
        .await
    }
}
