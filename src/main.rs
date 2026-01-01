mod gemini;
mod ssh;
mod websocket;
mod state;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::{
    services::ServeDir,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::state::AppState;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gemini_co_cli=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Create shared application state
    let app_state = AppState::new();

    // Build the application routes
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/api/session/create", post(websocket::create_session_handler))
        .route("/api/ssh/connect", post(websocket::ssh_connect_handler))
        .route("/ws/gemini-terminal/:session_id", get(websocket::gemini_terminal_ws_handler))
        .route("/ws/ssh-terminal/:session_id", get(websocket::ssh_terminal_ws_handler))
        .route("/ws/commands/:session_id", get(websocket::command_approval_ws_handler))
        .nest_service("/static", ServeDir::new("static"))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    // Start the server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../static/index.html"))
}
