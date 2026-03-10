use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod error;
mod handlers;
mod models;
mod routes;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(routes::health::health_check))
        .route("/api/backtest/run", post(routes::backtest::run_backtest))
        .route("/api/strategies", get(routes::strategies::list_strategies))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Ferrotick Web Dashboard running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind TCP listener");

    axum::serve(listener, app).await.expect("axum server error");
}
