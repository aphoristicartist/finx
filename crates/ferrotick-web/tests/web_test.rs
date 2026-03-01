use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use ferrotick_web::routes::health::health_check;
use tower::ServiceExt;

#[tokio::test]
async fn test_health_check() {
    let app = axum::Router::new().route("/health", axum::routing::get(health_check));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
