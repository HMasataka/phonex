use std::sync::Arc;

use axum::{extract, routing::post, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
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
    let candidate = c.to_json().unwrap().candidate;

    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/candidate"))
        .json(&CandidateRequest {
            candidate: candidate,
        })
        .send()
        .await
        .map_err(PhonexError::SendHTTPRequest)?;
    println!("signal_candidate Response: {}", resp.status());

    Ok(())
}

async fn candidate(extract::Json(req): extract::Json<CandidateRequest>) -> () {
    let pc = {
        let pcm = PEER_CONNECTION_MUTEX.lock().await;
        pcm.clone().unwrap()
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

async fn sdp(extract::Json(req): extract::Json<RTCSessionDescription>) {
    let pc = {
        let pcm = PEER_CONNECTION_MUTEX.lock().await;
        pcm.clone().unwrap()
    };
    let addr = {
        let addr = ADDRESS.lock().await;
        addr.clone()
    };

    println!("Req: {:?}", req);

    if let Err(err) = pc.set_remote_description(req).await {
        panic!("{}", err);
    }

    // Create an answer to send to the other process
    let answer = match pc.create_answer(None).await {
        Ok(a) => a,
        Err(err) => panic!("{}", err),
    };

    let _resp = reqwest::Client::new()
        .post(format!("http://{addr}/sdp"))
        .json(&answer)
        .send()
        .await
        .map_err(PhonexError::SendHTTPRequest)
        .unwrap();

    // Sets the LocalDescription, and starts our UDP listeners
    if let Err(err) = pc.set_local_description(answer).await {
        panic!("{}", err);
    }

    let cs = PENDING_CANDIDATES.lock().await;
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
) -> Result<(), SpanErr<PhonexError>> {
    {
        let mut oa = ADDRESS.lock().await;
        oa.clone_from(&offer_address);
    }

    println!("Listening on http://{answer_address}");
    {
        let mut pcm = PEER_CONNECTION_MUTEX.lock().await;
        *pcm = Some(Arc::clone(peer_connection));
    }

    let app = Router::new()
        .route("/candidate", post(candidate))
        .route("/sdp", post(sdp));

    let listener = tokio::net::TcpListener::bind(answer_address)
        .await
        .map_err(PhonexError::BuildTcpListener)?;

    axum::serve(listener, app)
        .await
        .map_err(PhonexError::ServeHTTP)?;

    Ok(())
}
