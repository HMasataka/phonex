use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

use crate::r#match::{
    MatchCandidateResponse, MatchRequest, MatchResponse, MatchSessionDescriptionResponse,
};

pub struct Server {
    register_receiver: Cell<Receiver<MatchRequest>>,
    response_channels: Arc<Mutex<HashMap<String, Sender<MatchResponse>>>>,
}

impl Server {
    pub fn new(rx: Cell<Receiver<MatchRequest>>) -> Self {
        Self {
            register_receiver: rx,
            response_channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn serve(&mut self) {
        loop {
            tokio::select! {
                val = self.register_receiver.get_mut().recv() => {
                    let request = val.unwrap();
                    self.handle_request(request).await;
                }
            };
        }
    }

    pub async fn handle_request(&mut self, request: MatchRequest) {
        match request {
            MatchRequest::Register(value) => {
                let mut m = self.response_channels.lock().await;
                if m.contains_key(&value.id) {
                    return;
                }

                m.insert(value.id, value.chan);
            }
            MatchRequest::SessionDescription(value) => {
                let m = self.response_channels.lock().await;
                if m.contains_key(&value.target_id) {
                    return;
                }

                let chan = m.get(&value.target_id).unwrap();
                chan.send(MatchResponse::SessionDescription(
                    MatchSessionDescriptionResponse { sdp: value.sdp },
                ))
                .await
                .unwrap();
            }
            MatchRequest::Candidate(value) => {
                let m = self.response_channels.lock().await;
                if m.contains_key(&value.target_id) {
                    return;
                }

                let chan = m.get(&value.target_id).unwrap();
                chan.send(MatchResponse::Candidate(MatchCandidateResponse {
                    candidate: value.candidate,
                }))
                .await
                .unwrap();
            }
        }
    }
}
