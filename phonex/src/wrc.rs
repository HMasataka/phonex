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
use tokio::time::Duration;
use tracing::instrument;
use tracing_spanned::SpanErr;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::{OnMessageHdlrFn, OnOpenHdlrFn, RTCDataChannel};
use webrtc::ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit};
use webrtc::ice_transport::ice_gatherer::OnLocalCandidateHdlrFn;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::{math_rand_alpha, RTCPeerConnection};

const TARGET: &str = "2";

pub struct WebRTC {
    rx: Cell<Receiver<HandshakeRequest>>,
    tx: Sender<HandshakeResponse>,
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
                    let req = candidate
                        .to_json()
                        .map_err(PhonexError::ConvertToJson)
                        .unwrap();

                    tx.send(HandshakeResponse::CandidateResponse(CandidateResponse {
                        target_id: candidate.stats_id, // FIXME
                        candidate: req.candidate,
                    }))
                    .await
                    .map_err(PhonexError::SendHandshakeResponse)
                    .unwrap();
                }
            }
        })
    }))
}

#[instrument(skip_all, name = "on_open", level = "trace")]
fn on_open(data_channel: Arc<RTCDataChannel>) -> Result<OnOpenHdlrFn, SpanErr<PhonexError>> {
    Ok(Box::new(move || {
        println!("Data channel '{}'-'{}' open. Random messages will now be sent to any connected DataChannels every 5 seconds", data_channel.label(), data_channel.id());

        let dc = Arc::clone(&data_channel);
        Box::pin(async move {
            let mut result = Result::<usize, PhonexError>::Ok(0);

            while result.is_ok() {
                let timeout = tokio::time::sleep(Duration::from_secs(5));
                tokio::pin!(timeout);

                tokio::select! {
                    _ = timeout.as_mut() =>{
                        let message = math_rand_alpha(15);
                        println!("Sending '{message}'");
                        result = dc.send_text(message).await.map_err(PhonexError::SendMessage);
                    }
                };
            }
        })
    }))
}

#[instrument(skip_all, name = "on_message", level = "trace")]
fn on_message(data_channel_label: String) -> Result<OnMessageHdlrFn, SpanErr<PhonexError>> {
    Ok(Box::new(move |msg: DataChannelMessage| {
        let msg_str = String::from_utf8(msg.data.to_vec())
            .map_err(PhonexError::ConvertByteToString)
            .unwrap();
        println!("Message from DataChannel '{data_channel_label}': '{msg_str}'");
        Box::pin(async {})
    }))
}

impl WebRTC {
    #[instrument(skip_all, name = "webrtc_new", level = "trace")]
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

    #[instrument(skip_all, name = "webrtc_handshake", level = "trace")]
    pub async fn handshake(&mut self) -> Result<(), SpanErr<PhonexError>> {
        let pc = Arc::clone(&self.peer_connection);
        let tx1 = self.tx.clone();
        pc.on_ice_candidate(on_ice_candidate(
            tx1,
            Arc::clone(&self.peer_connection),
            Arc::clone(&self.pending_candidates),
        )?);

        let data_channel = pc
            .create_data_channel("data", None)
            .await
            .map_err(PhonexError::CreateNewDataChannel)?;

        data_channel.on_open(on_open(Arc::clone(&data_channel))?);
        data_channel.on_message(on_message(data_channel.label().to_owned())?);

        self.offer().await?;

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

    #[instrument(skip_all, name = "webrtc_offer", level = "trace")]
    async fn offer(&mut self) -> Result<(), SpanErr<PhonexError>> {
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
                    target_id: TARGET.into(),
                    sdp: offer,
                },
            ))
            .await
            .map_err(PhonexError::SendHandshakeResponse)?;

        Ok(())
    }

    #[instrument(skip_all, name = "webrtc_handle_handshake", level = "trace")]
    async fn handle_handshake(&mut self, msg: HandshakeRequest) -> ControlFlow<(), ()> {
        match msg {
            HandshakeRequest::SessionDescriptionRequest(v) => {
                let v = self.peer_connection.set_remote_description(v.sdp).await;
                if v.is_err() {
                    println!("set_remote_description err: {:?}", v.err());
                    return ControlFlow::Break(());
                }

                if self.peer_connection.local_description().await.is_none() {
                    let answer = self.peer_connection.create_answer(None).await.unwrap();

                    self.tx
                        .send(HandshakeResponse::SessionDescriptionResponse(
                            SessionDescriptionResponse {
                                target_id: TARGET.into(),
                                sdp: answer.clone(),
                            },
                        ))
                        .await
                        .map_err(PhonexError::SendHandshakeResponse)
                        .unwrap();

                    self.peer_connection
                        .set_local_description(answer)
                        .await
                        .map_err(PhonexError::SetLocalDescription)
                        .unwrap();
                }

                let pending_candidates = self.pending_candidates.lock().await;
                for candidate in &*pending_candidates {
                    let req = candidate
                        .to_json()
                        .map_err(PhonexError::ConvertToJson)
                        .unwrap();

                    self.tx
                        .send(HandshakeResponse::CandidateResponse(CandidateResponse {
                            target_id: TARGET.into(),
                            candidate: req.candidate,
                        }))
                        .await
                        .map_err(PhonexError::SendHandshakeResponse)
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
                    .map_err(PhonexError::AddIceCandidate)
                    .unwrap();
            }
        }

        ControlFlow::Continue(())
    }

    #[instrument(skip_all, name = "webrtc_close_connection", level = "trace")]
    pub async fn close_connection(self) {
        self.peer_connection.close().await.unwrap();
    }
}
