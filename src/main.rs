use lazy_limit::{Duration, RuleConfig, init_rate_limiter};
use router::AppState;
use std::net::Ipv4Addr;
use tokio::net::TcpListener;
use tracing::{error, info};

mod admin;
mod auth;
mod config;
mod crypto;
mod error;
mod id;
mod jwt;
mod router;
mod users;

#[tokio::main]
async fn main() {
    config::tracing::init_tracing();

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
    .await;

    let pool = match config::database::get_connection_pool(None).await {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to obtain database pool: {}", e);
            return;
        }
    };

    let state = AppState::new(pool);
    let trace_layer = config::tracing::get_trace_layer();
    let cors_layer = config::net::get_cors_layer();
    let rate_limiter_layer = tower::ServiceBuilder::new()
        .layer(real::RealIpLayer::default())
        .layer(axum_governor::GovernorLayer::default());

    let app_router = router::routes()
        .with_state(state)
        .layer(trace_layer)
        .layer(cors_layer)
        .layer(rate_limiter_layer)
        .into_make_service_with_connect_info::<std::net::SocketAddr>();

    let address = Ipv4Addr::UNSPECIFIED;
    let port = 3000;
    let address = format!("{}:{}", address, port);

    match TcpListener::bind(&address).await {
        Ok(tcp_listener) => {
            info!("listening on {}", tcp_listener.local_addr().unwrap());

            axum::serve(tcp_listener, app_router)
                .with_graceful_shutdown(config::tracing::shutdown_signal())
                .await
                .unwrap()
        }
        Err(..) => {
            error!("Could not bind to {}", address);
        }
    }
}
