use crate::err::PhonexError;
use crate::message::{
    CandidateResponse, HandshakeRequest, HandshakeResponse, SessionDescriptionResponse,
};
use std::cell::Cell;
use std::ops::ControlFlow;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tracing_spanned::SpanErr;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit};
use webrtc::ice_transport::ice_gatherer::OnLocalCandidateHdlrFn;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;

pub struct WebRTC {
    rx: Cell<Receiver<HandshakeRequest>>,
    tx: Sender<HandshakeResponse>,
    peer_connection: Arc<RTCPeerConnection>,
    pending_candidates: Arc<Mutex<Vec<RTCIceCandidate>>>,
}

async fn initialize_peer_connection() -> Result<Arc<RTCPeerConnection>, SpanErr<PhonexError>> {
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };

    let mut m = MediaEngine::default();
    m.register_default_codecs()
        .map_err(PhonexError::InitializeRegistry)?;

    let mut registry = Registry::new();
    registry =
        register_default_interceptors(registry, &mut m).map_err(PhonexError::InitializeRegistry)?;

    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    let peer_connection = Arc::new(
        api.new_peer_connection(config)
            .await
            .map_err(PhonexError::CreateNewPeerConnection)?,
    );

    Ok(peer_connection)
}

fn on_ice_candidate(
    tx: Sender<HandshakeResponse>,
    peer_connection: Arc<RTCPeerConnection>,
    pending_candidates: Arc<Mutex<Vec<RTCIceCandidate>>>,
) -> Result<OnLocalCandidateHdlrFn, SpanErr<PhonexError>> {
    Ok(Box::new(move |c: Option<RTCIceCandidate>| {
        println!("on ice candidate: {:?}", c);

        let peer_connection = peer_connection.clone();
        let pending_candidates = pending_candidates.clone();
        let tx = tx.clone();

        Box::pin(async move {
            if let Some(candidate) = c {
                let desc = peer_connection.remote_description().await;
                if desc.is_none() {
                    let mut cs = pending_candidates.lock().await;
                    cs.push(candidate);
                } else {
                    let req = candidate.to_json().unwrap();

                    tx.send(HandshakeResponse::CandidateResponse(CandidateResponse {
                        target_id: candidate.stats_id, // FIXME
                        candidate: req.candidate,
                    }))
                    .await
                    .unwrap();
                }
            }
        })
    }))
}

impl WebRTC {
    pub async fn new(
        rx: Cell<Receiver<HandshakeRequest>>,
        tx: Sender<HandshakeResponse>,
    ) -> Result<Self, SpanErr<PhonexError>> {
        let peer_connection = initialize_peer_connection().await?;
        let pending_candidates: Arc<Mutex<Vec<RTCIceCandidate>>> = Arc::new(Mutex::new(vec![]));

        Ok(Self {
            rx,
            tx,
            peer_connection,
            pending_candidates,
        })
    }

    pub async fn handshake(&mut self) -> Result<(), SpanErr<PhonexError>> {
        let pc = Arc::clone(&self.peer_connection);
        let tx1 = self.tx.clone();
        pc.on_ice_candidate(on_ice_candidate(
            tx1,
            Arc::clone(&self.peer_connection),
            Arc::clone(&self.pending_candidates),
        )?);

        loop {
            tokio::select! {
                val = self.rx.get_mut().recv()=> {
                    let request = val.unwrap();
                     if self.handle_handshake(request).await.is_break(){
                        break;
                    }
                }
            };
        }

        Ok(())
    }

    async fn offer(self) -> Result<(), SpanErr<PhonexError>> {
        let offer = self
            .peer_connection
            .create_offer(None)
            .await
            .map_err(PhonexError::CreateNewOffer)?;

        self.peer_connection
            .set_local_description(offer.clone())
            .await
            .map_err(PhonexError::SetLocalDescription)?;

        self.tx
            .send(HandshakeResponse::SessionDescriptionResponse(
                SessionDescriptionResponse {
                    target_id: "1".into(),
                    sdp: offer,
                },
            ))
            .await
            .unwrap();

        Ok(())
    }

    async fn handle_handshake(&mut self, msg: HandshakeRequest) -> ControlFlow<(), ()> {
        match msg {
            HandshakeRequest::SessionDescriptionRequest(v) => {
                let v = self.peer_connection.set_remote_description(v.sdp).await;
                if v.is_err() {
                    return ControlFlow::Break(());
                }

                let pending_candidates = self.pending_candidates.lock().await;
                for candidate in &*pending_candidates {
                    let req = candidate.to_json().unwrap();

                    self.tx
                        .send(HandshakeResponse::CandidateResponse(CandidateResponse {
                            target_id: "1".into(),
                            candidate: req.candidate,
                        }))
                        .await
                        .unwrap();
                }
            }
            HandshakeRequest::CandidateRequest(v) => {
                self.peer_connection
                    .add_ice_candidate(RTCIceCandidateInit {
                        candidate: v.candidate,
                        ..Default::default()
                    })
                    .await
                    .unwrap();
            }
        }

        ControlFlow::Continue(())
    }

    pub async fn close_connection(self) {
        self.peer_connection.close().await.unwrap();
    }
}
