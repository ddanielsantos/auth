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
mod router;
mod users;

#[tokio::main]
async fn main() {
    config::tracing::init_tracing();

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
    let app_router = router::routes().with_state(state).layer(trace_layer).layer(cors_layer);

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
