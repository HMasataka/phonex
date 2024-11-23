use std::sync::{Arc, Mutex};

use axum::{routing::post, Router};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use tracing_spanned::SpanErr;
use webrtc::{
    ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit},
    peer_connection::{sdp::session_description::RTCSessionDescription, RTCPeerConnection},
};

use crate::err::PhonexError;

lazy_static! {
    pub static ref PEER_CONNECTION_MUTEX: Arc<Mutex<Option<Arc<RTCPeerConnection>>>> =
        Arc::new(Mutex::new(None));
    static ref ADDRESS: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    pub static ref PENDING_CANDIDATES: Arc<Mutex<Vec<RTCIceCandidate>>> =
        Arc::new(Mutex::new(vec![]));
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRequest {
    pub candidate: String,
}

#[instrument(skip_all, name = "signal_candidate", level = "trace")]
pub async fn signal_candidate(addr: &str, c: &RTCIceCandidate) -> Result<(), SpanErr<PhonexError>> {
    let payload = c.to_json().unwrap().candidate;

    let _resp = reqwest::Client::new()
        .post(format!("http://{addr}/candidate"))
        .json(&payload)
        .send()
        .await
        .unwrap();

    Ok(())
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
    let pc = {
        let pcm = PEER_CONNECTION_MUTEX.lock().unwrap();
        pcm.clone().unwrap()
    };
    let addr = {
        let addr = ADDRESS.lock().unwrap();
        addr.clone()
    };

    let limit = 2048usize;
    let body = request.into_body();
    let bytes = axum::body::to_bytes(body, limit).await.unwrap();
    let body_str = String::from_utf8(bytes.to_vec()).unwrap();
    println!("Body: {}", body_str);

    let sdp = match serde_json::from_str::<RTCSessionDescription>(&body_str) {
        Ok(s) => s,
        Err(err) => panic!("{}", err),
    };

    if let Err(err) = pc.set_remote_description(sdp).await {
        panic!("{}", err);
    }

    // Create an answer to send to the other process
    let answer = match pc.create_answer(None).await {
        Ok(a) => a,
        Err(err) => panic!("{}", err),
    };

    // Send our answer to the HTTP server listening in the other process
    let payload = match serde_json::to_string(&answer) {
        Ok(p) => p,
        Err(err) => panic!("{}", err),
    };

    let _resp = reqwest::Client::new()
        .post(format!("http://{addr}/sdp"))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Sets the LocalDescription, and starts our UDP listeners
    if let Err(err) = pc.set_local_description(answer).await {
        panic!("{}", err);
    }

    let cs = PENDING_CANDIDATES.lock().unwrap();
    for c in &*cs {
        if let Err(err) = signal_candidate(&addr, c).await {
            panic!("{}", err);
        }
    }
}

pub async fn serve(
    offer_address: String,
    answer_address: String,
    peer_connection: &Arc<RTCPeerConnection>,
) {
    {
        let mut oa = ADDRESS.lock().unwrap();
        oa.clone_from(&offer_address);
    }

    println!("Listening on http://{answer_address}");
    {
        let mut pcm = PEER_CONNECTION_MUTEX.lock().unwrap();
        *pcm = Some(Arc::clone(peer_connection));
    }

    let app = Router::new()
        .route("/candidate", post(candidate))
        .route("/sdp", post(sdp));

    let listener = tokio::net::TcpListener::bind(answer_address).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}
