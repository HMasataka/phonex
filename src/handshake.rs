use std::sync::{Arc, Mutex};

use axum::{routing::post, Router};
use webrtc::peer_connection::RTCPeerConnection;

lazy_static! {
    static ref PEER_CONNECTION_MUTEX: Arc<Mutex<Option<Arc<RTCPeerConnection>>>> =
        Arc::new(Mutex::new(None));
}

async fn candidate(request: axum::http::Request<axum::body::Body>) -> () {
    let limit = 2048usize;
    let body = request.into_body();
    let bytes = axum::body::to_bytes(body, limit).await.unwrap();
    let body_str = String::from_utf8(bytes.to_vec()).unwrap();
    println!("Body: {}", body_str);
}

async fn sdp(request: axum::http::Request<axum::body::Body>) -> () {
    let limit = 2048usize;
    let body = request.into_body();
    let bytes = axum::body::to_bytes(body, limit).await.unwrap();
    let body_str = String::from_utf8(bytes.to_vec()).unwrap();
    println!("Body: {}", body_str);
}

pub async fn serve(addr: String) {
    let app = Router::new()
        .route("/candidate", post(candidate))
        .route("/sdp", post(sdp));

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
