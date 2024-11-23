use std::sync::{Arc, Mutex};

use axum::{routing::post, Router};
use serde::{Deserialize, Serialize};
use webrtc::{
    ice_transport::ice_candidate::RTCIceCandidateInit, peer_connection::RTCPeerConnection,
};

lazy_static! {
    pub static ref PEER_CONNECTION_MUTEX: Arc<Mutex<Option<Arc<RTCPeerConnection>>>> =
        Arc::new(Mutex::new(None));
    static ref ADDRESS: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRequest {
    pub candidate: String,
}

async fn candidate(request: axum::http::Request<axum::body::Body>) -> () {
    let pc = {
        let pcm = PEER_CONNECTION_MUTEX.lock().unwrap();
        pcm.clone().unwrap()
    };

    let limit = 2048usize;
    let body = request.into_body();
    let bytes = axum::body::to_bytes(body, limit).await.unwrap();
    let body_str = String::from_utf8(bytes.to_vec()).unwrap();

    let req = match serde_json::from_str::<CandidateRequest>(&body_str) {
        Ok(s) => s,
        Err(err) => panic!("{}", err),
    };

    println!("Req: {:?}", req);

    if let Err(err) = pc
        .add_ice_candidate(RTCIceCandidateInit {
            candidate: req.candidate,
            ..Default::default()
        })
        .await
    {
        panic!("{}", err);
    }
}

async fn sdp(request: axum::http::Request<axum::body::Body>) -> () {
    let limit = 2048usize;
    let body = request.into_body();
    let bytes = axum::body::to_bytes(body, limit).await.unwrap();
    let body_str = String::from_utf8(bytes.to_vec()).unwrap();
    println!("Body: {}", body_str);
}

pub async fn serve(offer_address: String, answer_address: String) {
    {
        let mut oa = ADDRESS.lock().unwrap();
        oa.clone_from(&offer_address);
    }

    let app = Router::new()
        .route("/candidate", post(candidate))
        .route("/sdp", post(sdp));

    let listener = tokio::net::TcpListener::bind(answer_address).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
