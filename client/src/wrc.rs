use crate::data_channel::initialize_data_channel;
use crate::err::PhonexError;
use crate::message::Candidate;
use crate::message::Handshake;
use crate::message::SessionDescription;
use std::cell::Cell;
use std::ops::ControlFlow;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tracing::instrument;
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

const TARGET: &str = "1";

pub struct WebRTC {
    handshake_receiver: Cell<Receiver<Handshake>>,
    handshake_sender: Sender<Handshake>,
    peer_connection: Arc<RTCPeerConnection>,
    pending_candidates: Arc<Mutex<Vec<RTCIceCandidate>>>,
}

#[instrument(skip_all, name = "initialize_peer_connection", level = "trace")]
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

#[instrument(skip_all, name = "on_ice_candidate", level = "trace")]
fn on_ice_candidate(
    handshake_sender: Sender<Handshake>,
    peer_connection: Arc<RTCPeerConnection>,
    pending_candidates: Arc<Mutex<Vec<RTCIceCandidate>>>,
) -> Result<OnLocalCandidateHdlrFn, SpanErr<PhonexError>> {
    Ok(Box::new(move |c: Option<RTCIceCandidate>| {
        println!("on ice candidate: {:?}", c);

        let peer_connection = peer_connection.clone();
        let pending_candidates = pending_candidates.clone();
        let handshake_sender = handshake_sender.clone();

        Box::pin(async move {
            if let Some(candidate) = c {
                let desc = peer_connection.remote_description().await;
                if desc.is_none() {
                    let mut cs = pending_candidates.lock().await;
                    cs.push(candidate);
                } else {
                    if let Err(e) = handle_candidate(handshake_sender, candidate).await {
                        println!("on ice candidate error: {e}");
                    };
                }
            }
        })
    }))
}

#[instrument(skip_all, name = "handle_session_description_message", level = "trace")]
async fn handle_candidate(
    handshake_sender: Sender<Handshake>,
    candidate: RTCIceCandidate,
) -> Result<(), PhonexError> {
    let req = candidate.to_json().map_err(PhonexError::ConvertToJson)?;

    handshake_sender
        .send(Handshake::Candidate(Candidate {
            target_id: candidate.stats_id, // FIXME
            candidate: req.candidate,
        }))
        .await
        .map_err(PhonexError::SendHandshakeResponse)?;

    Ok(())
}

impl WebRTC {
    #[instrument(skip_all, name = "webrtc_new", level = "trace")]
    pub async fn new(
        handshake_receiver: Cell<Receiver<Handshake>>,
        handshake_sender: Sender<Handshake>,
    ) -> Result<Self, SpanErr<PhonexError>> {
        let peer_connection = initialize_peer_connection().await?;
        let pending_candidates: Arc<Mutex<Vec<RTCIceCandidate>>> = Arc::new(Mutex::new(vec![]));

        Ok(Self {
            handshake_receiver,
            handshake_sender,
            peer_connection,
            pending_candidates,
        })
    }

    #[instrument(skip_all, name = "webrtc_handshake", level = "trace")]
    pub async fn handshake(&mut self) -> Result<(), SpanErr<PhonexError>> {
        let pc = Arc::clone(&self.peer_connection);
        let handshake_sender = self.handshake_sender.clone();

        pc.on_ice_candidate(on_ice_candidate(
            handshake_sender,
            Arc::clone(&self.peer_connection),
            Arc::clone(&self.pending_candidates),
        )?);

        initialize_data_channel(pc, "data").await?;

        self.offer().await?;

        loop {
            tokio::select! {
                val = self.handshake_receiver.get_mut().recv()=> {
                    let request = val.unwrap();
                     if self.handle_handshake(request).await.is_break(){
                        break;
                    }
                }
            };
        }

        Ok(())
    }

    #[instrument(skip_all, name = "webrtc_offer", level = "trace")]
    async fn offer(&mut self) -> Result<(), SpanErr<PhonexError>> {
        let offer = self
            .peer_connection
            .create_offer(None)
            .await
            .map_err(PhonexError::CreateNewOffer)?;

        self.handshake_sender
            .send(Handshake::SessionDescription(SessionDescription {
                target_id: TARGET.into(),
                sdp: offer.clone(),
            }))
            .await
            .map_err(PhonexError::SendHandshakeResponse)?;

        self.peer_connection
            .set_local_description(offer)
            .await
            .map_err(PhonexError::SetLocalDescription)?;

        Ok(())
    }

    #[instrument(skip_all, name = "webrtc_answer", level = "trace")]
    async fn answer(&mut self) -> Result<(), SpanErr<PhonexError>> {
        let answer = self
            .peer_connection
            .create_answer(None)
            .await
            .map_err(PhonexError::CreateNewAnswer)?;

        self.handshake_sender
            .send(Handshake::SessionDescription(SessionDescription {
                target_id: TARGET.into(),
                sdp: answer.clone(),
            }))
            .await
            .map_err(PhonexError::SendHandshakeResponse)?;

        self.peer_connection
            .set_local_description(answer)
            .await
            .map_err(PhonexError::SetLocalDescription)?;

        Ok(())
    }

    #[instrument(skip_all, name = "handle_pending_candidate", level = "trace")]
    async fn handle_pending_candidate(&mut self) -> Result<(), SpanErr<PhonexError>> {
        let pending_candidates = self.pending_candidates.lock().await;

        for candidate in &*pending_candidates {
            let req = candidate.to_json().map_err(PhonexError::ConvertToJson)?;

            self.handshake_sender
                .send(Handshake::Candidate(Candidate {
                    target_id: TARGET.into(),
                    candidate: req.candidate,
                }))
                .await
                .map_err(PhonexError::SendHandshakeResponse)?;
        }

        Ok(())
    }

    #[instrument(skip_all, name = "webrtc_handle_handshake", level = "trace")]
    async fn handle_handshake(&mut self, msg: Handshake) -> ControlFlow<(), ()> {
        match msg {
            Handshake::SessionDescription(v) => {
                let v = self.peer_connection.set_remote_description(v.sdp).await;
                if v.is_err() {
                    println!("set_remote_description err: {:?}", v.err());
                    return ControlFlow::Break(());
                }

                if self.peer_connection.local_description().await.is_none() {
                    let v = self.answer().await;
                    if v.is_err() {
                        println!("send answer err: {:?}", v.err());
                        return ControlFlow::Break(());
                    }
                }

                let v = self.handle_pending_candidate().await;
                if v.is_err() {
                    println!("handle pending candidate err: {:?}", v.err());
                    return ControlFlow::Break(());
                }
            }
            Handshake::Candidate(v) => {
                let v = self
                    .peer_connection
                    .add_ice_candidate(RTCIceCandidateInit {
                        candidate: v.candidate,
                        ..Default::default()
                    })
                    .await
                    .map_err(PhonexError::AddIceCandidate);
                if v.is_err() {
                    println!("add ice candidate err: {:?}", v.err());
                    return ControlFlow::Break(());
                }
            }
        }

        ControlFlow::Continue(())
    }

    #[instrument(skip_all, name = "webrtc_close_connection", level = "trace")]
    pub async fn close_connection(self) {
        self.peer_connection.close().await.unwrap();
    }
}
