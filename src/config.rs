pub mod database {
    use crate::config::env::Env;
    use sqlx::PgPool;
    use sqlx::pool::PoolOptions;
    use std::time::Duration;

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

mod env {
    pub struct Env {
        pub database_url: String,
    }

    impl Env {
        pub fn new() -> Self {
            Self {
                database_url: dotenvy::var("DATABASE_URL").expect("env: GOOGLE_TOKEN_URL must be set"),
            }
        }
    }

    impl Default for Env {
        fn default() -> Self {
            Self::new()
        }
    }
}

pub mod tracing {
    use tokio::signal;
    use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, HttpMakeClassifier, TraceLayer};
    use tracing::Level;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

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

    pub fn get_trace_layer() -> TraceLayer<HttpMakeClassifier> {
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_response(DefaultOnResponse::new().level(Level::INFO))
    }
}

pub mod net {
    use axum::http::Method;
    use tower_http::cors;
    use tower_http::cors::{AllowHeaders, CorsLayer};

    pub fn get_cors_layer() -> CorsLayer {
        CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(AllowHeaders::any())
            .allow_origin(cors::Any)
    }
}
